//! Built-in handling for multisig instructions without requiring an executor upgrade.

use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
    sync::LazyLock,
};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    ValidationFail,
    account::{AccountId, MultisigMember, MultisigPolicy},
    isi::{
        AddSignatory, InstructionBox, RemoveSignatory, SetAccountQuorum,
        error::{InstructionExecutionError, InvalidParameterError},
    },
    metadata::Metadata,
    name::Name,
    prelude::{Grant, Json, Level, Log, Register, Revoke},
    query::error::{FindError, QueryExecutionFail},
    role::{Role, RoleId},
    state_path::StatePath,
};
use iroha_executor_data_model::isi::multisig::{
    DEFAULT_MULTISIG_TTL_MS, MultisigAccountState, MultisigApprovalOutcomeStatusV1,
    MultisigApprovalOutcomeV1, MultisigApprove, MultisigCancel, MultisigInstructionBox,
    MultisigProposalState, MultisigProposalTerminalExecutionStateV1, MultisigProposalTerminalState,
    MultisigProposalTerminalStatus, MultisigProposalValue, MultisigPropose, MultisigRegister,
    MultisigSpec,
};
use mv::storage::StorageReadOnly;

use crate::{
    smartcontracts::Execute,
    smartcontracts::isi::domain::isi::ensure_controller_capabilities,
    state::{
        StateTransaction, WorldReadOnly, public_lane_reward_record_matches_key,
        public_lane_stake_share_matches_key, public_lane_validator_record_matches_key,
    },
};

const DELIMITER: char = '/';
const MULTISIG: &str = "multisig";
const MULTISIG_ACCOUNT_STATE: &str = "account";
const MULTISIG_PROPOSAL_STATE: &str = "proposal";
const MULTISIG_PROPOSAL_TERMINAL_STATE: &str = "proposal-terminal";
const MULTISIG_PROPOSAL_TERMINAL_EXECUTION_STATE: &str = "proposal-terminal-execution";
const MULTISIG_APPROVAL_OUTCOME_STATE: &str = "approval-outcome";
const MULTISIG_SIGNATORY_INDEX_STATE: &str = "signatory";
const MULTISIG_SIGNATORY: &str = "MULTISIG_SIGNATORY";
const DOMAINLESS_NAMESPACE: &str = "domainless";
const MAX_MULTISIG_DEFERRED_EXECUTION_DEPTH: usize = 64;
type MultisigDeferredExecutionId = (AccountId, HashOf<Vec<InstructionBox>>);
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

/// Return the concrete instructions a live multisig approval can execute.
///
/// This read-only projection is used by non-bypassable deferred-execution admission. Missing,
/// expired, terminal, or malformed proposals cannot execute instructions and therefore resolve to
/// `None`.
pub(crate) fn live_proposal_instructions_for_approval(
    state_transaction: &StateTransaction<'_, '_>,
    approve: &MultisigApprove,
) -> Option<(AccountId, Vec<InstructionBox>)> {
    let proposal = proposal_state(
        state_transaction,
        &approve.account,
        &approve.instructions_hash,
    )
    .ok()?;
    if now_ms(state_transaction) >= proposal.expires_at_ms || proposal.is_relayed == Some(true) {
        return None;
    }
    Some((proposal.multisig_account_id, proposal.instructions))
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

pub(crate) fn multisig_account_state_key(account: &AccountId) -> StatePath {
    StatePath::from_str(&format!(
        "{MULTISIG}{DELIMITER}{MULTISIG_ACCOUNT_STATE}{DELIMITER}{}",
        HashOf::new(account)
    ))
    .expect("multisig account state path must be valid")
}

fn multisig_signatory_index_key(signatory: &AccountId) -> StatePath {
    StatePath::from_str(&format!(
        "{MULTISIG}{DELIMITER}{MULTISIG_SIGNATORY_INDEX_STATE}{DELIMITER}{}",
        HashOf::new(&signatory.subject_id())
    ))
    .expect("multisig signatory state path must be valid")
}

fn multisig_proposal_state_prefix(account: &AccountId) -> StatePath {
    StatePath::from_str(&format!(
        "{MULTISIG}{DELIMITER}{MULTISIG_PROPOSAL_STATE}{DELIMITER}{}{DELIMITER}",
        HashOf::new(account)
    ))
    .expect("multisig proposal state prefix must be valid")
}

fn multisig_proposal_state_key(
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
) -> StatePath {
    StatePath::from_str(&format!(
        "{}{}",
        multisig_proposal_state_prefix(multisig_account),
        instructions_hash
    ))
    .expect("constant string must be a valid state path")
}

fn multisig_proposal_terminal_state_prefix(account: &AccountId) -> StatePath {
    StatePath::from_str(&format!(
        "{MULTISIG}{DELIMITER}{MULTISIG_PROPOSAL_TERMINAL_STATE}{DELIMITER}{}{DELIMITER}",
        HashOf::new(account)
    ))
    .expect("multisig proposal terminal state prefix must be valid")
}

fn multisig_proposal_terminal_state_key(
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
) -> StatePath {
    StatePath::from_str(&format!(
        "{}{}",
        multisig_proposal_terminal_state_prefix(multisig_account),
        instructions_hash
    ))
    .expect("multisig proposal terminal state path must be valid")
}

fn multisig_proposal_terminal_execution_state_key(
    terminal_entrypoint_hash: [u8; Hash::LENGTH],
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
) -> StatePath {
    let terminal_entrypoint_hash = Hash::prehashed(terminal_entrypoint_hash);
    StatePath::from_str(&format!(
        "{MULTISIG}{DELIMITER}{MULTISIG_PROPOSAL_TERMINAL_EXECUTION_STATE}{DELIMITER}{terminal_entrypoint_hash}{DELIMITER}{}{DELIMITER}{instructions_hash}",
        HashOf::new(multisig_account),
    ))
    .expect("multisig proposal terminal execution path must be valid")
}

fn multisig_approval_outcome_state_key(
    entrypoint_hash: [u8; Hash::LENGTH],
    entrypoint_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
) -> StatePath {
    let entrypoint_hash = Hash::prehashed(entrypoint_hash);
    StatePath::from_str(&format!(
        "{MULTISIG}{DELIMITER}{MULTISIG_APPROVAL_OUTCOME_STATE}{DELIMITER}{entrypoint_hash}{DELIMITER}{}{DELIMITER}{instructions_hash}",
        HashOf::new(entrypoint_account),
    ))
    .expect("multisig approval outcome path must be valid")
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
        .get(old_account)
        .cloned()
        .ok_or_else(|| InstructionExecutionError::Find(FindError::Account(old_account.clone())))?;

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
            .account_aliases
            .view()
            .iter()
            .filter(|(_, account_id)| *account_id == old_account)
            .map(|(label, _)| label.clone()),
    );
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

    // Alias bindings and authoritative SNS ownership are one invariant. Scan the authoritative
    // namespace, rather than only the binding indexes, so acquired-but-unbound and
    // binding-cleared leases are migrated too. Preflight every update before mutating state.
    let alias_lease_updates = crate::sns::prepare_all_account_alias_lease_rekeys(
        state_transaction,
        old_account,
        new_account,
    )
    .map_err(|err| {
        InstructionExecutionError::InvariantViolation(
            format!("cannot preflight account-alias leases for rekey: {err}").into(),
        )
    })?;
    let now_ms = state_transaction.block_unix_timestamp_ms();
    for label in &labels_to_repoint {
        let Some((_, record)) = alias_lease_updates.get(label) else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("cannot rekey account alias `{label:?}`: authoritative SNS lease is missing or owned by another account").into(),
            ));
        };
        if !matches!(
            crate::sns::effective_status(record, now_ms),
            iroha_data_model::sns::NameStatus::Active
        ) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "cannot rekey account alias `{label:?}`: authoritative SNS lease is not active"
                )
                .into(),
            ));
        }
    }

    // Build every continuity update before changing the account table. A malformed or missing
    // record must fail this canonical rekey atomically instead of leaving a partially moved
    // account when this helper is exercised directly.
    let mut rekey_record_updates = Vec::with_capacity(labels_to_repoint.len());
    for label in &labels_to_repoint {
        let record = state_transaction
            .world
            .account_rekey_records
            .get(label)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot rekey account alias `{label:?}` without its canonical continuity record"
                    )
                    .into(),
                )
            })?;
        if &record.label != label || &record.active_account_id != old_account {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "cannot rekey account alias `{label:?}` whose continuity record does not target `{old_account}`"
                )
                .into(),
            ));
        }
        let record = record
            .repoint_for_account_id_rekey(new_account.clone())
            .map_err(|error| {
                InstructionExecutionError::InvariantViolation(
                    format!("cannot extend malformed account rekey history: {error}").into(),
                )
            })?;
        rekey_record_updates.push((label.clone(), record));
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

    let assets_to_move: Vec<_> = state_transaction
        .world
        .assets_in_account_iter(old_account)
        .map(|asset| asset.id().clone())
        .collect();
    for asset_id in &assets_to_move {
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
    }

    state_transaction
        .world
        .accounts
        .remove(old_account.clone())
        .expect("account existence was preflighted");
    state_transaction
        .world
        .accounts
        .insert(new_account.clone(), account_value.clone());

    for (label, record) in rekey_record_updates {
        state_transaction
            .world
            .insert_account_alias_binding(label.clone(), new_account.clone());
        state_transaction
            .world
            .account_rekey_records
            .insert(label, record);
    }
    for (_, (storage_key, record)) in alias_lease_updates {
        state_transaction
            .world
            .smart_contract_state
            .insert(storage_key, norito::codec::Encode::encode(&record));
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

    for asset_id in assets_to_move {
        let new_asset_id = iroha_data_model::asset::AssetId::with_scope(
            asset_id.definition().clone(),
            new_account.clone(),
            *asset_id.scope(),
        );
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
        .nfts_in_account_iter(old_account)
        .map(|nft| nft.id().clone())
        .collect();
    for nft_id in nft_ids {
        if let Some(value) = state_transaction.world.nfts.get_mut(&nft_id) {
            value.owned_by = new_account.clone();
        }
        state_transaction
            .world
            .replace_nft_owner_index(&nft_id, old_account, new_account);
    }

    let domain_ids: Vec<_> = state_transaction
        .world
        .domains_owned_by_iter(old_account)
        .map(|domain| domain.id.clone())
        .collect();
    for domain_id in domain_ids {
        if let Some(domain) = state_transaction.world.domains.get_mut(&domain_id) {
            domain.owned_by = new_account.clone();
        }
        state_transaction
            .world
            .replace_domain_owner_index(&domain_id, old_account, new_account);
    }

    let asset_def_ids: Vec<_> = state_transaction
        .world
        .asset_definitions_owned_by_iter(old_account)
        .map(|definition| definition.id.clone())
        .collect();
    for asset_def_id in asset_def_ids {
        if let Some(definition) = state_transaction
            .world
            .asset_definitions
            .get_mut(&asset_def_id)
        {
            definition.owned_by = new_account.clone();
        }
        state_transaction
            .world
            .replace_asset_definition_owner_index(&asset_def_id, old_account, new_account);
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
        .map_err(map_validation_fail)?;
    if old_account.multisig_policy().is_some() && previous_state.is_none() {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "multisig account `{old_account}` is missing its canonical native account state"
            )
            .into(),
        ));
    }
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
    let _ = (state_transaction, old, new);
}

fn replace_account_id_in_public_lane(
    state_transaction: &mut StateTransaction<'_, '_>,
    old: &AccountId,
    new: &AccountId,
) {
    let mut validator_updates = Vec::new();
    for (key, record) in state_transaction.world.public_lane_validators.iter() {
        if public_lane_validator_record_matches_key(key, record) && key.1 == *old {
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
        if let Some(record) = state_transaction.world.public_lane_validators.get_mut(&key)
            && public_lane_validator_record_matches_key(&key, record)
        {
            replace_account_id(&mut record.validator, old, new);
            replace_account_id(&mut record.stake_account, old, new);
        }
    }

    let mut stake_updates = Vec::new();
    for (key, record) in state_transaction.world.public_lane_stake_shares.iter() {
        if !public_lane_stake_share_matches_key(key, record) {
            continue;
        }
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
        if let Some(record) = state_transaction.world.public_lane_rewards.get_mut(&key)
            && public_lane_reward_record_matches_key(&key, record)
        {
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
    let receipt_ids: Vec<_> = state_transaction
        .world
        .settlement_receipts
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    for receipt_id in receipt_ids {
        if let Some(receipt) = state_transaction
            .world
            .settlement_receipts
            .get_mut(&receipt_id)
        {
            replace_account_id(&mut receipt.authority, old, new);
            for leg in &mut receipt.legs {
                replace_account_id(&mut leg.leg.from, old, new);
                replace_account_id(&mut leg.leg.to, old, new);
            }
            if let Some(fx_corridor) = receipt.fx_corridor.as_mut() {
                replace_account_id(&mut fx_corridor.source_account, old, new);
                replace_account_id(&mut fx_corridor.source_sink, old, new);
                replace_account_id(&mut fx_corridor.destination_reserve, old, new);
                replace_account_id(&mut fx_corridor.recipient, old, new);
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
        let expected_account = AccountId::new_multisig(
            multisig_policy_from_spec(&spec).map_err(ValidationFail::InstructionFailed)?,
        );
        if expected_account != multisig_account_id {
            return Err(ValidationFail::NotPermitted(format!(
                "multisig account `{multisig_account_id}` already exists and cannot be rekeyed to `{expected_account}` by registration"
            )));
        }
        let previous_state = load_multisig_account_state_optional(
            state_transaction,
            &multisig_account_id,
        )?
        .ok_or_else(|| {
            ValidationFail::NotPermitted(format!(
                "multisig account `{multisig_account_id}` already exists without canonical native account state"
            ))
        })?;
        if previous_state.home_domain != home_domain {
            return Err(ValidationFail::NotPermitted(format!(
                "multisig account `{multisig_account_id}` already exists with a different home domain"
            )));
        }
        if previous_state.spec.signatories != spec.signatories
            || previous_state.spec.quorum != spec.quorum
        {
            return Err(ValidationFail::NotPermitted(format!(
                "multisig account `{multisig_account_id}` already exists with a different signatory policy"
            )));
        }
        let next_state = MultisigAccountState::new(multisig_account_id.clone(), home_domain, spec);
        reconcile_multisig_transition(
            authority,
            state_transaction,
            &multisig_account_id,
            Some(&previous_state),
            Some(&next_state),
        )?;
        return Ok(());
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
    let multisig_account = match resolve_signatory_account(state_transaction, &instruction.account)
    {
        Ok(value) => value,
        Err(err) => {
            iroha_logger::error!(
                proposer = %proposer,
                requested_multisig_account = %instruction.account,
                error = ?err,
                "multisig propose failed to resolve multisig account"
            );
            return Err(err);
        }
    };
    let home_domain = match multisig_home_domain(state_transaction, &multisig_account) {
        Ok(value) => value,
        Err(err) => {
            iroha_logger::error!(
                proposer = %proposer,
                multisig_account = %multisig_account,
                error = ?err,
                "multisig propose failed to load multisig home domain"
            );
            return Err(err);
        }
    };
    let instructions_hash = HashOf::new(&instruction.instructions);
    let multisig_spec = match multisig_spec(state_transaction, &multisig_account) {
        Ok(value) => value,
        Err(err) => {
            iroha_logger::error!(
                proposer = %proposer,
                multisig_account = %multisig_account,
                instructions_hash = %instructions_hash,
                error = ?err,
                "multisig propose failed to load multisig spec"
            );
            return Err(err);
        }
    };
    let home_domain_literal = home_domain
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".to_owned());
    iroha_logger::info!(
        proposer = %proposer,
        multisig_account = %multisig_account,
        instructions_hash = %instructions_hash,
        instruction_count = instruction.instructions.len(),
        home_domain = %home_domain_literal,
        quorum = multisig_spec.quorum.get(),
        ttl_ms = multisig_spec.transaction_ttl_ms.get(),
        signatory_count = multisig_spec.signatories.len(),
        "multisig propose evaluating proposal"
    );
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
        iroha_logger::error!(
            proposer = %proposer,
            multisig_account = %multisig_account,
            instructions_hash = %instructions_hash,
            requested_ttl_ms = ?instruction.transaction_ttl_ms,
            spec_ttl_ms = multisig_spec.transaction_ttl_ms.get(),
            "multisig propose rejected because ttl exceeds multisig spec"
        );
        return Err(ValidationFail::NotPermitted(
            "ttl violates the restriction".to_owned(),
        ));
    }

    if !(is_downward_proposal || has_multisig_role || is_signatory || is_self_proposal) {
        iroha_logger::error!(
            proposer = %proposer,
            multisig_account = %multisig_account,
            instructions_hash = %instructions_hash,
            proposer_role = ?proposer_role,
            multisig_role = ?multisig_role,
            is_downward_proposal,
            has_multisig_role,
            is_signatory,
            is_self_proposal,
            "multisig propose rejected because proposer is not authorized"
        );
        return Err(ValidationFail::NotPermitted(
            "not qualified to propose multisig".to_owned(),
        ));
    }

    match proposal_state(state_transaction, &multisig_account, &instructions_hash) {
        Ok(existing) if now_ms(state_transaction) < existing.expires_at_ms => {
            iroha_logger::warn!(
                proposer = %proposer,
                multisig_account = %multisig_account,
                instructions_hash = %instructions_hash,
                now_ms = now_ms(state_transaction),
                expires_at_ms = existing.expires_at_ms,
                "multisig propose rejected as duplicate active proposal"
            );
            return Err(ValidationFail::NotPermitted(
                "multisig proposal duplicates".to_owned(),
            ));
        }
        Ok(_) => {}
        Err(ValidationFail::QueryFailed(QueryExecutionFail::NotFound)) => {}
        Err(err) => {
            iroha_logger::error!(
                proposer = %proposer,
                multisig_account = %multisig_account,
                instructions_hash = %instructions_hash,
                error = ?err,
                "multisig propose failed while checking existing proposal state"
            );
            return Err(err);
        }
    }

    let now_ms = now_ms(state_transaction);
    if proposal_state(state_transaction, &multisig_account, &instructions_hash).is_ok() {
        if let Err(err) = prune_expired(
            state_transaction,
            &multisig_account,
            &instructions_hash,
            &instruction.account,
        ) {
            iroha_logger::error!(
                proposer = %proposer,
                multisig_account = %multisig_account,
                instructions_hash = %instructions_hash,
                error = ?err,
                "multisig propose failed while pruning expired proposal state"
            );
            return Err(err);
        }
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
    let resolved_signatories = match resolved_signatory_accounts(state_transaction, &multisig_spec)
    {
        Ok(value) => value,
        Err(err) => {
            iroha_logger::error!(
                proposer = %proposer,
                multisig_account = %multisig_account,
                instructions_hash = %instructions_hash,
                error = ?err,
                "multisig propose failed to resolve signatory accounts"
            );
            return Err(err);
        }
    };
    let resolved_signatory_summary = resolved_signatories
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    iroha_logger::info!(
        proposer = %proposer,
        multisig_account = %multisig_account,
        instructions_hash = %instructions_hash,
        resolved_signatory_count = resolved_signatories.len(),
        resolved_signatories = %resolved_signatory_summary,
        "multisig propose resolved signatory accounts"
    );
    for signatory in resolved_signatories {
        let signatory_is_multisig = match is_multisig(state_transaction, &signatory) {
            Ok(value) => value,
            Err(err) => {
                iroha_logger::error!(
                    proposer = %proposer,
                    multisig_account = %multisig_account,
                    instructions_hash = %instructions_hash,
                    signatory = %signatory,
                    error = ?err,
                    "multisig propose failed while checking signatory multisig state"
                );
                return Err(err);
            }
        };
        if signatory_is_multisig
            && let Err(err) = deploy_relayer(
                state_transaction,
                &signatory,
                &approve_me,
                now_ms,
                expires_at_ms,
            )
        {
            iroha_logger::error!(
                proposer = %proposer,
                multisig_account = %multisig_account,
                instructions_hash = %instructions_hash,
                relayer = %signatory,
                error = ?err,
                "multisig propose failed to deploy nested multisig relayer"
            );
            return Err(err);
        }
    }

    let proposal_state = MultisigProposalState::new(
        multisig_account,
        instructions_hash,
        proposal_value.instructions,
        proposal_value.proposed_at_ms,
        proposal_value.expires_at_ms,
        proposal_value.approvals,
        proposal_value.is_relayed,
    );
    match store_multisig_proposal_state(state_transaction, &proposal_state) {
        Ok(()) => {
            iroha_logger::info!(
                proposer = %proposer,
                multisig_account = %proposal_state.multisig_account_id,
                instructions_hash = %proposal_state.instructions_hash,
                proposed_at_ms = proposal_state.proposed_at_ms,
                expires_at_ms = proposal_state.expires_at_ms,
                approvals = proposal_state.approvals.len(),
                "multisig propose stored proposal state"
            );
            Ok(())
        }
        Err(err) => {
            iroha_logger::error!(
                proposer = %proposer,
                multisig_account = %proposal_state.multisig_account_id,
                instructions_hash = %proposal_state.instructions_hash,
                error = ?err,
                "multisig propose failed to store proposal state"
            );
            Err(err)
        }
    }
}

fn execute_approve(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &MultisigApprove,
) -> Result<(), ValidationFail> {
    let approver = authority.clone();
    let multisig_account = resolve_signatory_account(state_transaction, &instruction.account)?;
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
    if let Err(err) = prune_expired(
        state_transaction,
        &multisig_account,
        &instructions_hash,
        &instruction.account,
    ) {
        iroha_logger::error!(
            multisig_account = %multisig_account,
            instructions_hash = %instructions_hash,
            approver = %authority,
            error = ?err,
            "multisig approval proposal lookup failed before approval"
        );
        return Err(err);
    }

    let Ok(mut proposal_state) =
        proposal_state(state_transaction, &multisig_account, &instructions_hash)
    else {
        store_multisig_approval_outcome(
            state_transaction,
            &instruction.account,
            &multisig_account,
            &instructions_hash,
            MultisigApprovalOutcomeStatusV1::NotExecuted,
        )?;
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
        store_multisig_approval_outcome(
            state_transaction,
            &instruction.account,
            &multisig_account,
            &instructions_hash,
            MultisigApprovalOutcomeStatusV1::NotExecuted,
        )?;
        return Ok(());
    }

    upsert_subject_approval(&mut proposal_state.approvals, approver);
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

    if !is_authenticated {
        iroha_logger::info!(
            multisig_account = %multisig_account,
            instructions_hash = %instructions_hash,
            approvals = proposal_state.approvals.len(),
            "multisig approval storing updated proposal state"
        );
        store_multisig_proposal_state(state_transaction, &proposal_state)?;
        store_multisig_approval_outcome(
            state_transaction,
            &instruction.account,
            &multisig_account,
            &instructions_hash,
            MultisigApprovalOutcomeStatusV1::NotExecuted,
        )?;
        iroha_logger::info!(
            multisig_account = %multisig_account,
            instructions_hash = %instructions_hash,
            "multisig approval stored updated proposal state"
        );
        return Ok(());
    }

    if is_authenticated {
        crate::validation_fee::enforce_deferred_instruction_list(
            &multisig_account,
            proposal_state.instructions.as_slice(),
            state_transaction,
        )
        .map_err(|reason| ValidationFail::NotPermitted(reason.to_string()))?;

        let execution_id = (multisig_account.clone(), instructions_hash);
        begin_multisig_deferred_execution(
            &mut state_transaction.multisig_deferred_execution_stack,
            &execution_id,
        )?;
        let executor = state_transaction.world.executor.clone();
        let execution_result = (|| {
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
                        &instruction.account,
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
                    maybe_store_relayed_proposal_execution_state(
                        state_transaction,
                        &proposal_state,
                        &instruction.account,
                    )?;
                    store_multisig_proposal_state(state_transaction, &proposal_state)?;
                }
                _ => unreachable!("proposal_state.is_relayed checked above"),
            }

            for instruction in proposal_state.instructions {
                let instruction_debug = format!("{instruction:?}");
                iroha_logger::info!(
                    multisig_account = %multisig_account,
                    instructions_hash = %instructions_hash,
                    approver = %authority,
                    instruction = %instruction_debug,
                    "multisig approval executing authenticated instruction"
                );
                if let Err(err) =
                    executor.execute_instruction(state_transaction, &multisig_account, instruction)
                {
                    iroha_logger::error!(
                        multisig_account = %multisig_account,
                        instructions_hash = %instructions_hash,
                        approver = %authority,
                        instruction = %instruction_debug,
                        error = ?err,
                        "multisig approval authenticated instruction failed"
                    );
                    return Err(err);
                }
                iroha_logger::info!(
                    multisig_account = %multisig_account,
                    instructions_hash = %instructions_hash,
                    approver = %authority,
                    "multisig approval finished authenticated instruction"
                );
            }

            store_multisig_approval_outcome(
                state_transaction,
                &instruction.account,
                &multisig_account,
                &instructions_hash,
                MultisigApprovalOutcomeStatusV1::Executed,
            )
        })();
        finish_multisig_deferred_execution(
            &mut state_transaction.multisig_deferred_execution_stack,
            &execution_id,
        );
        execution_result?;
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
    let instructions_hash = instruction.instructions_hash;

    if !canceler_is_authorized(&multisig_account, &canceler) {
        return Err(ValidationFail::NotPermitted(
            "multisig cancel must execute as the multisig account".to_owned(),
        ));
    }

    prune_expired(
        state_transaction,
        &multisig_account,
        &instructions_hash,
        &instruction.account,
    )?;

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
        &instruction.account,
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

fn begin_multisig_deferred_execution(
    execution_stack: &mut Vec<MultisigDeferredExecutionId>,
    execution_id: &MultisigDeferredExecutionId,
) -> Result<(), ValidationFail> {
    if execution_stack.iter().any(|active| active == execution_id) {
        return Err(ValidationFail::NotPermitted(
            "multisig deferred execution contains a proposal cycle".to_owned(),
        ));
    }
    if execution_stack.len() >= MAX_MULTISIG_DEFERRED_EXECUTION_DEPTH {
        return Err(ValidationFail::TooComplex);
    }
    execution_stack.push(execution_id.clone());
    Ok(())
}

fn finish_multisig_deferred_execution(
    execution_stack: &mut Vec<MultisigDeferredExecutionId>,
    execution_id: &MultisigDeferredExecutionId,
) {
    let finished = execution_stack.pop();
    debug_assert_eq!(finished.as_ref(), Some(execution_id));
}

fn prune_expired(
    state_transaction: &mut StateTransaction<'_, '_>,
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
    entrypoint_account: &AccountId,
) -> Result<(), ValidationFail> {
    prune_expired_with_guard(
        state_transaction,
        multisig_account,
        instructions_hash,
        entrypoint_account,
        0,
        &mut BTreeSet::new(),
    )
}

fn prune_expired_with_guard(
    state_transaction: &mut StateTransaction<'_, '_>,
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
    entrypoint_account: &AccountId,
    depth: usize,
    active_path: &mut BTreeSet<MultisigDeferredExecutionId>,
) -> Result<(), ValidationFail> {
    if depth >= MAX_MULTISIG_DEFERRED_EXECUTION_DEPTH {
        return Err(ValidationFail::TooComplex);
    }
    let proposal_state = proposal_state(state_transaction, multisig_account, instructions_hash)?;
    let execution_id = (
        proposal_state.multisig_account_id.clone(),
        proposal_state.instructions_hash,
    );
    if !active_path.insert(execution_id.clone()) {
        return Err(ValidationFail::NotPermitted(
            "multisig proposal expiry traversal contains a cycle".to_owned(),
        ));
    }

    let result = (|| {
        if now_ms(state_transaction) < proposal_state.expires_at_ms {
            return Ok(());
        }

        for instruction in &proposal_state.instructions {
            if let Ok(MultisigInstructionBox::Approve(approve)) =
                MultisigInstructionBox::try_from(instruction)
            {
                prune_expired_with_guard(
                    state_transaction,
                    &approve.account,
                    &approve.instructions_hash,
                    &approve.account,
                    depth.saturating_add(1),
                    active_path,
                )?;
            }
        }

        maybe_store_terminal_proposal_state(
            state_transaction,
            &proposal_state,
            MultisigProposalTerminalStatus::Expired,
            entrypoint_account,
        )?;
        prune_down(state_transaction, multisig_account, instructions_hash)
    })();
    active_path.remove(&execution_id);
    result
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
    if state.account_id != resolved_account {
        return Err(ValidationFail::QueryFailed(QueryExecutionFail::Conversion(
            format!(
                "native multisig account state is bound to `{}`, not `{resolved_account}`",
                state.account_id
            ),
        )));
    }
    ensure_quorum_reachable(&state.spec)?;
    ensure_signatories_are_single(&state.spec)?;
    let expected_account = AccountId::new_multisig(
        multisig_policy_from_spec(&state.spec).map_err(ValidationFail::InstructionFailed)?,
    );
    if expected_account != resolved_account {
        return Err(ValidationFail::QueryFailed(QueryExecutionFail::Conversion(
            format!(
                "native multisig account state policy derives `{expected_account}`, not `{resolved_account}`"
            ),
        )));
    }

    let account = state_transaction
        .world
        .account(&resolved_account)
        .map_err(map_find_error)?;
    if let Some(metadata_spec) = account.metadata().get(&spec_key()).cloned() {
        let metadata_spec = metadata_spec
            .try_into_any_norito::<MultisigSpec>()
            .map_err(|err| {
                ValidationFail::QueryFailed(QueryExecutionFail::Conversion(format!(
                    "invalid multisig/spec metadata for `{resolved_account}`: {err}"
                )))
            })?;
        if metadata_spec != state.spec {
            return Err(ValidationFail::QueryFailed(QueryExecutionFail::Conversion(
                format!(
                    "multisig/spec metadata disagrees with canonical native account state for `{resolved_account}`"
                ),
            )));
        }
    }
    if let Some(metadata_home_domain) = account.metadata().get(&home_domain_key()).cloned() {
        let metadata_home_domain = metadata_home_domain
            .try_into_any_norito::<Option<iroha_data_model::domain::DomainId>>()
            .map_err(|err| {
                ValidationFail::QueryFailed(QueryExecutionFail::Conversion(format!(
                    "invalid multisig home-domain metadata for `{resolved_account}`: {err}"
                )))
            })?;
        if metadata_home_domain != state.home_domain {
            return Err(ValidationFail::QueryFailed(QueryExecutionFail::Conversion(
                format!(
                    "multisig home-domain metadata disagrees with canonical native account state for `{resolved_account}`"
                ),
            )));
        }
    }
    Ok(Some(state))
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

fn store_multisig_proposal_terminal_execution_state(
    state_transaction: &mut StateTransaction<'_, '_>,
    terminal_state: &MultisigProposalTerminalState,
    entrypoint_account: &AccountId,
) -> Result<(), ValidationFail> {
    let Some(terminal_entrypoint_hash) = state_transaction
        .tx_call_hash
        .as_ref()
        .map(|hash| *hash.as_ref())
    else {
        // Some unit-level execution helpers do not install a transaction call hash. There is no
        // transaction identity to bind immutable evidence to in that context.
        return Ok(());
    };
    let execution_state = MultisigProposalTerminalExecutionStateV1::new(
        terminal_state.clone(),
        entrypoint_account.clone(),
        state_transaction.block_height(),
        terminal_entrypoint_hash,
    );
    let bytes = norito::to_bytes(&execution_state).map_err(multisig_state_encode_error)?;
    let execution_key = multisig_proposal_terminal_execution_state_key(
        terminal_entrypoint_hash,
        entrypoint_account,
        &terminal_state.instructions_hash,
    );
    if let Some(existing) = state_transaction
        .world
        .smart_contract_state
        .get(&execution_key)
    {
        if existing.as_slice() != bytes {
            return Err(ValidationFail::InternalError(format!(
                "conflicting immutable multisig terminal execution state at `{execution_key}`"
            )));
        }
    } else {
        state_transaction
            .world
            .smart_contract_state
            .insert(execution_key, bytes);
    }
    Ok(())
}

fn store_multisig_approval_outcome(
    state_transaction: &mut StateTransaction<'_, '_>,
    entrypoint_account: &AccountId,
    resolved_multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
    status: MultisigApprovalOutcomeStatusV1,
) -> Result<(), ValidationFail> {
    let Some(entrypoint_hash) = state_transaction
        .tx_call_hash
        .as_ref()
        .map(|hash| *hash.as_ref())
    else {
        // Unit-level helpers without a transaction identity cannot emit durable approval evidence.
        return Ok(());
    };
    let outcome = MultisigApprovalOutcomeV1::new(
        entrypoint_account.clone(),
        resolved_multisig_account.clone(),
        *instructions_hash,
        status,
        state_transaction.block_height(),
        entrypoint_hash,
    );
    let bytes = norito::to_bytes(&outcome).map_err(multisig_state_encode_error)?;
    let outcome_key =
        multisig_approval_outcome_state_key(entrypoint_hash, entrypoint_account, instructions_hash);
    if let Some(existing) = state_transaction
        .world
        .smart_contract_state
        .get(&outcome_key)
    {
        if existing.as_slice() != bytes {
            return Err(ValidationFail::InternalError(format!(
                "conflicting immutable multisig approval outcome at `{outcome_key}`"
            )));
        }
    } else {
        state_transaction
            .world
            .smart_contract_state
            .insert(outcome_key, bytes);
    }
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
    entrypoint_account: &AccountId,
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
    store_multisig_proposal_terminal_execution_state(
        state_transaction,
        &terminal_state,
        entrypoint_account,
    )?;
    store_multisig_proposal_terminal_state(state_transaction, &terminal_state)
}

fn maybe_store_relayed_proposal_execution_state(
    state_transaction: &mut StateTransaction<'_, '_>,
    proposal_state: &MultisigProposalState,
    entrypoint_account: &AccountId,
) -> Result<(), ValidationFail> {
    debug_assert_eq!(proposal_state.is_relayed, Some(true));
    let terminal_state = MultisigProposalTerminalState::new(
        proposal_state.multisig_account_id.clone(),
        proposal_state.instructions_hash,
        proposal_state_value(proposal_state),
        MultisigProposalTerminalStatus::Finalized,
        now_ms(state_transaction),
    );
    store_multisig_proposal_terminal_execution_state(
        state_transaction,
        &terminal_state,
        entrypoint_account,
    )
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

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        ChainId, IntoKeyValue, Registrable,
        account::{
            Account, AccountController, AccountId, MultisigMember, MultisigPolicy,
            rekey::{AccountAlias, AccountAliasDomain, AccountRekeyTransitionProvenance},
        },
        alias_setup::{
            AccountAliasRoleV1, AccountProvisionV1, AliasAccountIntentV1, AliasIntentV1,
            AliasLeaseAcquisitionV1, AliasQuoteGuardV1, ResolvedAccountAliasV1,
        },
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        block::BlockHeader,
        domain::DomainId,
        isi::{
            AddSignatory, ExecuteTrigger, Grant, Mint, RemoveSignatory, SetAccountQuorum,
            SetKeyValue,
            alias_setup::EnsureAlias,
            settlement::{
                FxCorridorSettlementDetails, SettlementKind, SettlementLeg, SettlementLegRole,
                SettlementLegSnapshot, SettlementPlan, SettlementReceipt,
            },
        },
        nexus::{DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, UniversalAccountId},
        permission::Permission,
        prelude::{Domain, InstructionBox, Quantity, Register},
        transaction::IvmBytecode,
    };
    use iroha_executor_data_model::isi::multisig::{
        DEFAULT_MULTISIG_TTL_MS, MultisigApprove, MultisigCancel, MultisigPropose,
        MultisigRegister, MultisigSpec,
    };
    use iroha_executor_data_model::permission::account::{
        AccountAliasPermissionScope, CanManageAccountAlias, CanRegisterAccount,
    };
    use mv::storage::StorageReadOnly;
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        executor::Executor,
        kura::Kura,
        query::store::LiveQueryStore,
        sns::{
            SnsNamespace, get_name_record, policy_by_id, quote_resolved_name_registration,
            seed_default_namespace_policies, sync_default_namespace_policy_payment_asset,
        },
        state::{State, World},
    };

    fn new_account_id(key_pair: &KeyPair) -> AccountId {
        AccountId::new(key_pair.public_key().clone())
    }

    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("multisig ISI fixture key generation should succeed")
    }

    #[test]
    fn checked_keypair_helper_preserves_default_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
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
        let multisig_key = checked_keypair();
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

    fn install_trigger_contract(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        signing_keypair: &KeyPair,
        code: Vec<u8>,
        mut manifest: iroha_data_model::smart_contract::manifest::ContractManifest,
        nonce: u64,
    ) -> (
        IvmBytecode,
        iroha_data_model::smart_contract::ContractAddress,
    ) {
        let code_hash = ivm::contract_code_hash(&code);
        let bytecode = IvmBytecode::from_compiled(code.clone());
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            authority,
            nonce,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive trigger contract address");
        Register::account(iroha_data_model::account::NewAccount::new(
            contract_address.subject_id(),
        ))
        .execute(authority, state_transaction)
        .expect("register trigger contract subject");
        let deployment_permission: Permission =
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode
                .into();
        Grant::account_permission(deployment_permission, authority.clone())
            .execute(authority, state_transaction)
            .expect("grant trigger contract deployment permission");
        let registered_hash =
            crate::smartcontracts::code::register_code_bytes(authority, code, state_transaction)
                .expect("register trigger contract bytecode");
        assert_eq!(registered_hash, code_hash);
        manifest.code_hash = Some(code_hash);
        crate::smartcontracts::code::register_manifest(
            authority,
            manifest.signed(signing_keypair),
            state_transaction,
        )
        .expect("register trigger contract manifest");
        crate::smartcontracts::code::activate_instance(
            authority,
            contract_address.clone(),
            code_hash,
            state_transaction,
        )
        .expect("activate trigger contract");
        (bytecode, contract_address)
    }

    fn bind_account_label(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        account_id: &AccountId,
        domain_id: &iroha_data_model::domain::DomainId,
        label: &str,
    ) -> AccountAlias {
        bind_account_label_in_dataspace(
            state_transaction,
            authority,
            account_id,
            domain_id,
            DataSpaceId::UNIVERSAL,
            label,
        )
    }

    fn bind_account_label_in_dataspace(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        account_id: &AccountId,
        domain_id: &iroha_data_model::domain::DomainId,
        dataspace: DataSpaceId,
        label: &str,
    ) -> AccountAlias {
        let _ = authority;
        let label = AccountAlias::new(
            label.parse().expect("account label name"),
            Some(AccountAliasDomain::new(domain_id.name().clone())),
            dataspace,
        );
        let selector = crate::sns::selector_for_account_alias(
            &label,
            &state_transaction.nexus.dataspace_catalog,
        )
        .expect("account alias selector");
        let address = iroha_data_model::account::AccountAddress::from_account_id(account_id)
            .expect("account address");
        let lease = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            account_id.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            Metadata::default(),
        );
        state_transaction.world.smart_contract_state.insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&lease),
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

    fn account_alias_lease_record(
        state_transaction: &StateTransaction<'_, '_>,
        alias: &AccountAlias,
    ) -> iroha_data_model::sns::NameRecordV1 {
        let selector = crate::sns::selector_for_account_alias(
            alias,
            &state_transaction.nexus.dataspace_catalog,
        )
        .expect("account alias selector");
        let bytes = state_transaction
            .world
            .smart_contract_state
            .get(&crate::sns::record_storage_key(&selector))
            .expect("account alias lease");
        let mut slice = bytes.as_slice();
        let record = norito::codec::Decode::decode(&mut slice).expect("decode account alias lease");
        assert!(slice.is_empty(), "account alias lease must be canonical");
        record
    }

    fn assert_account_rekey_not_applied(
        state_transaction: &StateTransaction<'_, '_>,
        old_account: &AccountId,
        new_account: &AccountId,
        aliases: &[AccountAlias],
    ) {
        assert!(
            state_transaction.world.account(old_account).is_ok(),
            "failed rekey must retain the old account"
        );
        assert!(
            state_transaction.world.account(new_account).is_err(),
            "failed rekey must not materialize the new account"
        );
        for alias in aliases {
            assert_eq!(
                state_transaction.world.account_aliases.get(alias),
                Some(old_account),
                "failed rekey must preserve alias target"
            );
            assert_eq!(
                state_transaction
                    .world
                    .account_rekey_records
                    .get(alias)
                    .expect("account rekey record")
                    .active_account_id,
                *old_account,
                "failed rekey must preserve the active rekey-record account"
            );
        }
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

    fn seed_domain_name_lease_tx(
        state_transaction: &mut StateTransaction<'_, '_>,
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
        state_transaction.world.smart_contract_state.insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
    }

    fn durable_int_value(bytes: &[u8]) -> i64 {
        use ivm::state_value::{
            StateValueAtomV1, StateValueKindV1, StateValueNodeV1, StateValueRecordV1,
            StateValueSchemaV1, state_value_schema_hash_v1,
        };

        // Typed durable state stores a schema-bound Norito record. The authenticated
        // pointer-ABI envelope is the record's leaf atom, not the outer storage bytes.
        let schema = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Int)],
        };
        let schema_bytes = norito::to_bytes(&schema).expect("encode durable int schema");
        let record: StateValueRecordV1 =
            norito::decode_from_bytes(bytes).expect("decode durable int state record");
        assert_eq!(
            norito::to_bytes(&record).expect("re-encode durable int state record"),
            bytes,
            "durable int state record must use canonical Norito encoding"
        );
        assert_eq!(
            record.schema_hash,
            state_value_schema_hash_v1(&schema_bytes),
            "durable int state record must bind the exact Int schema"
        );
        assert!(
            schema.validate_atoms(&record.atoms),
            "durable int state record must match the Int atom stream"
        );
        let [StateValueAtomV1::Pointer(envelope)] = record.atoms.as_slice() else {
            panic!("durable int state record must contain one pointer atom");
        };
        ivm::numeric_tlv::decode_int_bytes(envelope)
            .expect("decode canonical durable int pointer")
            .try_to_i64()
            .expect("test durable int value fits i64")
    }

    fn durable_state_values_under_contract_prefix(
        state_transaction: &StateTransaction<'_, '_>,
        contract_address: &iroha_data_model::smart_contract::ContractAddress,
        prefix: &str,
    ) -> Vec<Vec<u8>> {
        let scope_digest = hex::encode(Hash::new(contract_address.to_string().as_bytes()).as_ref());
        let physical_prefix = format!("sc/{scope_digest}/{prefix}");
        let prefix_with_child = format!("{physical_prefix}/");
        state_transaction
            .world
            .smart_contract_state
            .iter()
            .filter_map(|(key, value)| {
                let key = key.as_ref();
                (key == physical_prefix || key.starts_with(prefix_with_child.as_str()))
                    .then(|| value.clone())
            })
            .collect()
    }

    fn register_domain_with_name_lease(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        domain_id: &iroha_data_model::domain::DomainId,
        label: &str,
    ) {
        seed_domain_name_lease_tx(state_transaction, authority, domain_id);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(authority, state_transaction)
            .expect(label);
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("acme", "universal").unwrap();

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);

        register_domain_with_name_lease(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            "domain registration",
        );

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
        let multisig_account_key = checked_keypair();
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
    fn multisig_executes_fi_registration_alias_batch_as_multisig_authority() {
        assert_multisig_executes_fi_registration_alias_batch(false);
    }

    #[test]
    fn multisig_executes_fi_registration_alias_batch_with_uaid_account() {
        assert_multisig_executes_fi_registration_alias_batch(true);
    }

    fn assert_multisig_executes_fi_registration_alias_batch(include_registration_uaid: bool) {
        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        let retail_account = new_account_id(&checked_keypair());
        let sbp = DataSpaceId::new(10);
        let hbl_domain = DomainId::try_new("hbl", "sbp").expect("hbl.sbp domain");
        let payment_asset_definition_id: AssetDefinitionId = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
            .parse()
            .expect("payment asset definition id");
        let mut world = World::with(
            [Domain::new(hbl_domain.clone()).build(&signer1_id)],
            [
                Account::new(signer1_id.clone()).build(&signer1_id),
                Account::new(signer2_id.clone()).build(&signer1_id),
            ],
            [
                AssetDefinition::numeric(payment_asset_definition_id.clone())
                    .with_name("xor".to_owned())
                    .build(&signer1_id),
            ],
        );
        seed_default_namespace_policies(&mut world);
        assert!(
            sync_default_namespace_policy_payment_asset(
                &mut world,
                &payment_asset_definition_id.to_string()
            ),
            "fixture SNS policies must use the configured Nexus fee asset"
        );
        let state = State::new_with_chain(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("multisig-fi-registration-alias-batch"),
        );
        state.nexus.write().dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: sbp,
                alias: "sbp".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("sbp dataspace catalog");
        state.nexus.write().fees.fee_asset_id = payment_asset_definition_id.to_string();

        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        execute_register(
            &mut state_transaction,
            &signer1_id,
            MultisigRegister::with_account(
                new_account_id(&checked_keypair()),
                hbl_domain.clone(),
                spec.clone(),
            ),
        )
        .expect("register multisig");
        let multisig_id =
            AccountId::new_multisig(multisig_policy_from_spec(&spec).expect("policy"));
        state_transaction.world.add_account_permission(
            &multisig_id,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(sbp),
            }),
        );
        state_transaction.world.add_account_permission(
            &multisig_id,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(hbl_domain.clone()),
            }),
        );
        state_transaction.world.add_account_permission(
            &multisig_id,
            Permission::from(CanRegisterAccount {
                domain: hbl_domain.clone(),
            }),
        );
        Mint::asset_quantity(
            1_000_u64,
            AssetId::of(payment_asset_definition_id.clone(), multisig_id.clone()),
        )
        .execute(&signer1_id, &mut state_transaction)
        .expect("mint payment balance");

        let alias = AccountAlias::new(
            "clear-orbit-3941".parse().expect("label"),
            Some(AccountAliasDomain::new("hbl".parse().expect("domain"))),
            sbp,
        );
        let registration_uaid = include_registration_uaid.then(|| {
            UniversalAccountId::from_hash(Hash::new(
                b"retail_registration|alias=clear-orbit-3941@hbl.sbp",
            ))
        });
        let registration_account = if let Some(uaid) = registration_uaid {
            Account::new(retail_account.clone()).with_uaid(Some(uaid))
        } else {
            Account::new(retail_account.clone())
        };
        let resolved_alias = ResolvedAccountAliasV1::new(
            "clear-orbit-3941@hbl.sbp"
                .parse()
                .expect("resolved FI account alias"),
            sbp,
        );
        let selector = crate::alias_setup::selector_for_resolved_alias_target(
            &iroha_data_model::alias_setup::AliasTargetV1::AccountAlias(resolved_alias.clone()),
        )
        .expect("FI account alias selector");
        let quote = quote_resolved_name_registration(
            state_transaction.world(),
            selector,
            &retail_account,
            1,
            None,
            state_transaction.block_unix_timestamp_ms(),
        )
        .expect("FI account alias quote");
        let policy_version = policy_by_id(
            state_transaction.world(),
            iroha_data_model::sns::ACCOUNT_ALIAS_SUFFIX_ID,
        )
        .expect("account alias policy")
        .policy_version;
        let instructions = vec![
            InstructionBox::from(Register::account(registration_account)),
            InstructionBox::from(EnsureAlias::new(
                AliasIntentV1::AccountAlias(AliasAccountIntentV1 {
                    alias: resolved_alias,
                    target_account: retail_account.clone(),
                    provision: AccountProvisionV1::Existing,
                    role: AccountAliasRoleV1::Additional,
                }),
                AliasLeaseAcquisitionV1::new(1, None),
                AliasQuoteGuardV1 {
                    expected_policy_version: policy_version,
                    expected_payment_asset: payment_asset_definition_id.clone(),
                    max_amount: quote.charge_amount,
                    valid_until_ms: u64::MAX,
                },
            )),
        ];
        let instructions_hash = HashOf::new(&instructions);
        state_transaction.tx_call_hash = Some(Hash::prehashed([0xD3; Hash::LENGTH]));
        execute_propose(
            &mut state_transaction,
            &signer1_id,
            &MultisigPropose::new(multisig_id.clone(), instructions, None),
        )
        .expect("signer1 proposes FI registration batch");
        state_transaction.tx_call_hash = Some(Hash::prehashed([0xD4; Hash::LENGTH]));
        execute_approve(
            &mut state_transaction,
            &signer2_id,
            &MultisigApprove::new(multisig_id.clone(), instructions_hash),
        )
        .expect("signer2 approval executes FI registration batch");

        let lease = get_name_record(
            state_transaction.world(),
            &state_transaction.nexus.dataspace_catalog,
            SnsNamespace::AccountAlias,
            "clear-orbit-3941@hbl.sbp",
            0,
        )
        .expect("FI alias lease should be active after multisig execution");
        assert_eq!(lease.owner, retail_account);
        assert_eq!(
            state_transaction.world.account_aliases().get(&alias),
            Some(&retail_account),
            "FI alias binding should be visible after multisig execution"
        );
        if let Some(uaid) = registration_uaid {
            assert_eq!(
                state_transaction.world.uaid_accounts.get(&uaid),
                Some(&retail_account),
                "UAID should be bound to the registered retail account"
            );
        }
    }

    #[test]
    fn register_existing_multisig_account_refreshes_ttl() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-register-refresh-ttl"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("acme", "universal").unwrap();

        let owner = checked_keypair();
        let owner_id = new_account_id(&owner);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer = checked_keypair();
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
        execute_register(
            &mut state_transaction,
            &owner_id,
            MultisigRegister::with_account(
                new_account_id(&checked_keypair()),
                domain_id.clone(),
                spec.clone(),
            ),
        )
        .expect("initial register");

        let registered_multisig_id =
            AccountId::new_multisig(multisig_policy_from_spec(&spec).expect("policy"));
        let refreshed_ttl = NonZeroU64::new(86_400_000).unwrap();
        let refreshed_spec = MultisigSpec {
            transaction_ttl_ms: refreshed_ttl,
            ..spec
        };
        execute_register(
            &mut state_transaction,
            &owner_id,
            MultisigRegister::with_account(
                registered_multisig_id.clone(),
                domain_id.clone(),
                refreshed_spec.clone(),
            ),
        )
        .expect("refresh existing multisig ttl");

        let stored_spec = multisig_spec(&state_transaction, &registered_multisig_id)
            .expect("refreshed spec must decode");
        assert_eq!(stored_spec.transaction_ttl_ms, refreshed_ttl);
        assert_eq!(stored_spec.signatories, refreshed_spec.signatories);
        assert_eq!(stored_spec.quorum, refreshed_spec.quorum);
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("acme", "universal").unwrap();

        let owner = checked_keypair();
        let owner_id = new_account_id(&owner);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let missing_signer = checked_keypair();
        let missing_signer_id = new_account_id(&missing_signer);
        let spec = MultisigSpec {
            signatories: BTreeMap::from([(owner_id.clone(), 1), (missing_signer_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };

        let multisig_seed = new_account_id(&checked_keypair());
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("acme", "universal").unwrap();

        let owner = checked_keypair();
        let owner_id = new_account_id(&owner);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let registrar = checked_keypair();
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
        let multisig_seed = new_account_id(&checked_keypair());

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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("acme", "universal").unwrap();

        let owner = checked_keypair();
        let owner_id = new_account_id(&owner);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer = checked_keypair();
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
        let multisig_seed = new_account_id(&checked_keypair());
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("acme", "universal").unwrap();

        let owner = checked_keypair();
        let owner_id = new_account_id(&owner);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let missing_signer = checked_keypair();
        let missing_signer_id = new_account_id(&missing_signer);
        let invalid_spec = MultisigSpec {
            signatories: BTreeMap::from([(owner_id.clone(), 1), (missing_signer_id.clone(), 1)]),
            quorum: NonZeroU16::new(3).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_seed = new_account_id(&checked_keypair());

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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("acme", "universal").unwrap();

        let owner = checked_keypair();
        let owner_id = new_account_id(&owner);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let missing_signer = checked_keypair();
        let missing_signer_id = new_account_id(&missing_signer);
        let spec = MultisigSpec {
            signatories: BTreeMap::from([(owner_id.clone(), 1), (missing_signer_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_seed = new_account_id(&checked_keypair());
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("wonderland", "universal").unwrap();

        let owner_key = checked_keypair();
        let owner_id = new_account_id(&owner_key);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("wonderland", "universal").unwrap();

        let owner_key = checked_keypair();
        let owner_id = new_account_id(&owner_key);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
        let signer3 = checked_keypair();
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
        assert_eq!(
            rekey_record.transition_provenance,
            vec![AccountRekeyTransitionProvenance::AccountIdRekey],
            "canonical multisig rekey must record trusted account-id provenance"
        );
        let lease = account_alias_lease_record(&state_transaction, &alias);
        assert_eq!(
            lease.owner, updated_account,
            "authoritative SNS ownership must follow the canonical multisig account id"
        );
        let old_address = iroha_data_model::account::AccountAddress::from_account_id(&multisig_id)
            .expect("old multisig address");
        let new_address =
            iroha_data_model::account::AccountAddress::from_account_id(&updated_account)
                .expect("updated multisig address");
        assert!(
            lease
                .controllers
                .iter()
                .any(|controller| controller.account_address.as_ref() == Some(&new_address)),
            "SNS owner controller must follow the canonical multisig account id"
        );
        assert!(
            lease
                .controllers
                .iter()
                .all(|controller| controller.account_address.as_ref() != Some(&old_address)),
            "SNS controllers must not retain the obsolete multisig account id"
        );
        let proposal = proposal_value(&state_transaction, &updated_account, &instructions_hash)
            .expect("proposal should move to the rekeyed account");
        assert_eq!(
            proposal.approvals,
            BTreeSet::from([signer1_id.clone()]),
            "existing approvals should survive add-signatory rekey"
        );

        let approval_entrypoint_hash = Hash::prehashed([0xd1; Hash::LENGTH]);
        state_transaction.tx_call_hash = Some(approval_entrypoint_hash);
        execute_approve(
            &mut state_transaction,
            &signer2_id,
            &MultisigApprove::new(updated_account.clone(), instructions_hash),
        )
        .expect("approval through rekeyed account");

        let outcome_key = multisig_approval_outcome_state_key(
            *approval_entrypoint_hash.as_ref(),
            &updated_account,
            &instructions_hash,
        );
        let outcome_bytes = state_transaction
            .world
            .smart_contract_state
            .get(&outcome_key)
            .expect("rekeyed approval should leave an exact outcome");
        let outcome = norito::decode_from_bytes::<MultisigApprovalOutcomeV1>(outcome_bytes)
            .expect("rekeyed approval outcome should decode");
        assert_eq!(outcome.entrypoint_account_id, updated_account);
        assert_eq!(outcome.resolved_multisig_account_id, updated_account);
        assert_eq!(outcome.status, MultisigApprovalOutcomeStatusV1::Executed);

        let terminal_execution_key = multisig_proposal_terminal_execution_state_key(
            *approval_entrypoint_hash.as_ref(),
            &updated_account,
            &instructions_hash,
        );
        let terminal_execution_bytes = state_transaction
            .world
            .smart_contract_state
            .get(&terminal_execution_key)
            .expect("rekeyed finalization should use its signed lookup account");
        let terminal_execution = norito::decode_from_bytes::<
            MultisigProposalTerminalExecutionStateV1,
        >(terminal_execution_bytes)
        .expect("rekeyed terminal execution should decode");
        assert_eq!(terminal_execution.entrypoint_account_id, updated_account);
        assert_eq!(
            terminal_execution.terminal.multisig_account_id,
            updated_account
        );
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("wonderland", "universal").unwrap();

        let owner_key = checked_keypair();
        let owner_id = new_account_id(&owner_key);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1 = checked_keypair();
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

        let missing_signer = checked_keypair();
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("wonderland", "universal").unwrap();

        let owner_key = checked_keypair();
        let owner_id = new_account_id(&owner_key);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("wonderland", "universal").unwrap();

        let owner_key = checked_keypair();
        let owner_id = new_account_id(&owner_key);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
        let signer3 = checked_keypair();
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("wonderland", "universal").unwrap();

        let owner_key = checked_keypair();
        let owner_id = new_account_id(&owner_key);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("wonderland", "universal").unwrap();

        let owner_key = checked_keypair();
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

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
        let signer3 = checked_keypair();
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
    fn rekey_account_id_preflights_every_alias_lease_without_partial_mutation() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-rekey-alias-lease-preflight"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id = DomainId::try_new("rekey", "universal").expect("domain id");
        let old_account = new_account_id(&checked_keypair());
        let new_account = new_account_id(&checked_keypair());
        let foreign_owner = new_account_id(&checked_keypair());

        register_domain_with_name_lease(
            &mut state_transaction,
            &old_account,
            &domain_id,
            "register rekey domain",
        );
        register_account_in_domain(
            &mut state_transaction,
            &old_account,
            &domain_id,
            &old_account,
            "register old account",
        );
        let aliases = [
            "a_missing",
            "b_foreign",
            "c_inactive",
            "d_malformed",
            "e_duplicate",
        ]
        .map(|label| {
            bind_account_label(
                &mut state_transaction,
                &old_account,
                &old_account,
                &domain_id,
                label,
            )
        });
        let lease_keys = aliases.clone().map(|alias| {
            let selector = crate::sns::selector_for_account_alias(
                &alias,
                &state_transaction.nexus.dataspace_catalog,
            )
            .expect("selector");
            crate::sns::record_storage_key(&selector)
        });
        let canonical_leases = lease_keys.clone().map(|key| {
            state_transaction
                .world
                .smart_contract_state
                .get(&key)
                .expect("canonical lease")
                .clone()
        });

        state_transaction
            .world
            .smart_contract_state
            .remove(lease_keys[0].clone());
        let err = rekey_account_id(
            &mut state_transaction,
            &old_account,
            &new_account,
            Some(&domain_id),
        )
        .expect_err("missing lease must reject account rekey");
        assert!(
            err.to_string()
                .contains("missing or owned by another account"),
            "{err}"
        );
        assert_account_rekey_not_applied(&state_transaction, &old_account, &new_account, &aliases);
        state_transaction
            .world
            .smart_contract_state
            .insert(lease_keys[0].clone(), canonical_leases[0].clone());

        let mut foreign_lease = account_alias_lease_record(&state_transaction, &aliases[1]);
        foreign_lease.owner = foreign_owner;
        state_transaction.world.smart_contract_state.insert(
            lease_keys[1].clone(),
            norito::codec::Encode::encode(&foreign_lease),
        );
        let err = rekey_account_id(
            &mut state_transaction,
            &old_account,
            &new_account,
            Some(&domain_id),
        )
        .expect_err("foreign-owned lease must reject account rekey");
        assert!(
            err.to_string().contains("owned by another account"),
            "{err}"
        );
        assert_account_rekey_not_applied(&state_transaction, &old_account, &new_account, &aliases);
        state_transaction
            .world
            .smart_contract_state
            .insert(lease_keys[1].clone(), canonical_leases[1].clone());

        let mut inactive_lease = account_alias_lease_record(&state_transaction, &aliases[2]);
        inactive_lease.expires_at_ms = 0;
        state_transaction.world.smart_contract_state.insert(
            lease_keys[2].clone(),
            norito::codec::Encode::encode(&inactive_lease),
        );
        let err = rekey_account_id(
            &mut state_transaction,
            &old_account,
            &new_account,
            Some(&domain_id),
        )
        .expect_err("inactive lease must reject account rekey");
        assert!(err.to_string().contains("not active"), "{err}");
        assert_account_rekey_not_applied(&state_transaction, &old_account, &new_account, &aliases);
        state_transaction
            .world
            .smart_contract_state
            .insert(lease_keys[2].clone(), canonical_leases[2].clone());

        let mut malformed_lease = canonical_leases[3].clone();
        malformed_lease.push(0xFF);
        state_transaction
            .world
            .smart_contract_state
            .insert(lease_keys[3].clone(), malformed_lease);
        let err = rekey_account_id(
            &mut state_transaction,
            &old_account,
            &new_account,
            Some(&domain_id),
        )
        .expect_err("non-canonical lease bytes must reject account rekey");
        let error_text = err.to_string();
        assert!(
            error_text.contains("failed to decode SNS lease")
                || error_text.contains("failed to decode account-alias SNS record")
                || error_text.contains("trailing bytes"),
            "{err}"
        );
        assert_account_rekey_not_applied(&state_transaction, &old_account, &new_account, &aliases);
        state_transaction
            .world
            .smart_contract_state
            .insert(lease_keys[3].clone(), canonical_leases[3].clone());

        let mut duplicate_replacement_lease =
            account_alias_lease_record(&state_transaction, &aliases[4]);
        let new_address = iroha_data_model::account::AccountAddress::from_account_id(&new_account)
            .expect("new account address");
        duplicate_replacement_lease.controllers.push(
            iroha_data_model::sns::NameControllerV1::account(&new_address),
        );
        state_transaction.world.smart_contract_state.insert(
            lease_keys[4].clone(),
            norito::codec::Encode::encode(&duplicate_replacement_lease),
        );
        let err = rekey_account_id(
            &mut state_transaction,
            &old_account,
            &new_account,
            Some(&domain_id),
        )
        .expect_err("pre-existing replacement controller must reject account rekey");
        assert!(
            err.to_string()
                .contains("already contains the replacement account controller"),
            "{err}"
        );
        assert_account_rekey_not_applied(&state_transaction, &old_account, &new_account, &aliases);
        state_transaction
            .world
            .smart_contract_state
            .insert(lease_keys[4].clone(), canonical_leases[4].clone());

        let canonical_rekey_record = state_transaction
            .world
            .account_rekey_records
            .get(&aliases[0])
            .cloned()
            .expect("canonical continuity record");
        state_transaction
            .world
            .account_rekey_records
            .remove(aliases[0].clone());
        let err = rekey_account_id(
            &mut state_transaction,
            &old_account,
            &new_account,
            Some(&domain_id),
        )
        .expect_err("missing continuity record must reject account rekey");
        assert!(
            err.to_string().contains("canonical continuity record"),
            "{err}"
        );
        state_transaction
            .world
            .account_rekey_records
            .insert(aliases[0].clone(), canonical_rekey_record.clone());
        assert_account_rekey_not_applied(&state_transaction, &old_account, &new_account, &aliases);

        let mut malformed_rekey_record = canonical_rekey_record.clone();
        malformed_rekey_record
            .transition_provenance
            .push(AccountRekeyTransitionProvenance::LegacyUnspecified);
        state_transaction
            .world
            .account_rekey_records
            .insert(aliases[0].clone(), malformed_rekey_record);
        let err = rekey_account_id(
            &mut state_transaction,
            &old_account,
            &new_account,
            Some(&domain_id),
        )
        .expect_err("malformed continuity record must reject account rekey");
        assert!(
            err.to_string().contains("malformed account rekey history"),
            "{err}"
        );
        assert_account_rekey_not_applied(&state_transaction, &old_account, &new_account, &aliases);
        state_transaction
            .world
            .account_rekey_records
            .insert(aliases[0].clone(), canonical_rekey_record);

        rekey_account_id(
            &mut state_transaction,
            &old_account,
            &new_account,
            Some(&domain_id),
        )
        .expect("canonical active leases should migrate atomically");
        for alias in &aliases {
            assert_eq!(
                state_transaction.world.account_aliases.get(alias),
                Some(&new_account)
            );
            let lease = account_alias_lease_record(&state_transaction, alias);
            assert_eq!(lease.owner, new_account);
            let new_address =
                iroha_data_model::account::AccountAddress::from_account_id(&new_account)
                    .expect("new account address");
            assert!(
                lease
                    .controllers
                    .iter()
                    .any(|controller| controller.account_address.as_ref() == Some(&new_address))
            );
        }
    }

    #[test]
    fn rekey_account_id_migrates_acquired_unbound_and_binding_cleared_leases() {
        let state = State::new_with_chain(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("multisig-rekey-unbound-alias-leases"),
        );
        let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 10, 0));
        let mut state_transaction = block.transaction();
        let domain_id = DomainId::try_new("unbound", "universal").expect("domain id");
        let old_account = new_account_id(&checked_keypair());
        let new_account = new_account_id(&checked_keypair());
        register_domain_with_name_lease(
            &mut state_transaction,
            &old_account,
            &domain_id,
            "register domain",
        );
        register_account_in_domain(
            &mut state_transaction,
            &old_account,
            &domain_id,
            &old_account,
            "register account",
        );

        let aliases = ["acquired_unbound", "binding_cleared"].map(|label| {
            let alias = AccountAlias::new(
                label.parse().expect("alias label"),
                Some(AccountAliasDomain::new(domain_id.name().clone())),
                DataSpaceId::UNIVERSAL,
            );
            let selector = crate::sns::selector_for_account_alias(
                &alias,
                &state_transaction.nexus.dataspace_catalog,
            )
            .expect("selector");
            let address = iroha_data_model::account::AccountAddress::from_account_id(&old_account)
                .expect("old account address");
            let lease = iroha_data_model::sns::NameRecordV1::new(
                selector.clone(),
                old_account.clone(),
                vec![iroha_data_model::sns::NameControllerV1::account(&address)],
                0,
                0,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                Metadata::default(),
            );
            state_transaction.world.smart_contract_state.insert(
                crate::sns::record_storage_key(&selector),
                norito::codec::Encode::encode(&lease),
            );
            alias
        });
        for alias in &aliases {
            assert!(state_transaction.world.account_aliases.get(alias).is_none());
            assert!(
                state_transaction
                    .world
                    .account_rekey_records
                    .get(alias)
                    .is_none()
            );
        }

        rekey_account_id(
            &mut state_transaction,
            &old_account,
            &new_account,
            Some(&domain_id),
        )
        .expect("all owned leases should migrate even without bindings");

        assert!(state_transaction.world.account(&old_account).is_err());
        assert!(state_transaction.world.account(&new_account).is_ok());
        let new_address = iroha_data_model::account::AccountAddress::from_account_id(&new_account)
            .expect("new account address");
        for alias in &aliases {
            let lease = account_alias_lease_record(&state_transaction, alias);
            assert_eq!(lease.owner, new_account);
            assert_eq!(
                lease.controllers,
                vec![iroha_data_model::sns::NameControllerV1::account(
                    &new_address
                )]
            );
            assert!(
                state_transaction.world.account_aliases.get(alias).is_none(),
                "unbound leases must remain unbound"
            );
            assert!(
                state_transaction
                    .world
                    .account_rekey_records
                    .get(alias)
                    .is_none(),
                "unbound leases must not gain resolution records"
            );
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("default", "universal").unwrap();

        let old_key = checked_keypair();
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

        let new_key = checked_keypair();
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
    fn rekey_settlement_receipt_updates_fx_detail_accounts_with_legs() {
        let state = State::new_with_chain(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("multisig-rekey-settlement-receipt"),
        );
        let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
        let mut state_transaction = block.transaction();
        let old_account = new_account_id(&checked_keypair());
        let new_account = new_account_id(&checked_keypair());
        let counterparty = new_account_id(&checked_keypair());
        let domain_id = DomainId::try_new("fx", "universal").expect("domain id");
        let source_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "source".parse().expect("asset name"));
        let destination_asset_definition_id =
            AssetDefinitionId::new(domain_id, "destination".parse().expect("asset name"));
        let settlement_id: iroha_data_model::isi::settlement::SettlementId =
            "fx_rekey_receipt".parse().expect("settlement id");
        let receipt = SettlementReceipt {
            kind: SettlementKind::FxCorridor,
            authority: old_account.clone(),
            plan: SettlementPlan::default(),
            metadata: Metadata::default(),
            block_height: 1,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [0; Hash::LENGTH],
            )),
            executed_at_ms: 1,
            legs: [
                SettlementLegSnapshot {
                    role: SettlementLegRole::FxSource,
                    leg: SettlementLeg::new(
                        source_asset_definition_id.clone(),
                        Quantity::one(),
                        old_account.clone(),
                        counterparty.clone(),
                    ),
                },
                SettlementLegSnapshot {
                    role: SettlementLegRole::FxDestination,
                    leg: SettlementLeg::new(
                        destination_asset_definition_id.clone(),
                        Quantity::one(),
                        counterparty.clone(),
                        old_account.clone(),
                    ),
                },
            ],
            fx_corridor: Some(FxCorridorSettlementDetails {
                policy_id: "corridor".parse().expect("policy id"),
                policy_revision: 1,
                source_dataspace: DataSpaceId::new(1),
                destination_dataspace: DataSpaceId::new(2),
                rate_numerator: 1,
                rate_denominator: 1,
                source_account: old_account.clone(),
                source_sink: old_account.clone(),
                destination_reserve: old_account.clone(),
                recipient: old_account.clone(),
                source_asset_definition_id,
                destination_asset_definition_id,
                source_amount: Quantity::one(),
                destination_amount: Quantity::one(),
            }),
        };
        state_transaction
            .world
            .settlement_receipts
            .insert(settlement_id.clone(), receipt);

        replace_account_id_in_settlements(&mut state_transaction, &old_account, &new_account);

        let receipt = state_transaction
            .world
            .settlement_receipts
            .get(&settlement_id)
            .expect("rekeyed settlement receipt");
        assert_eq!(receipt.authority, new_account);
        assert_eq!(receipt.legs[0].leg.from(), &new_account);
        assert_eq!(receipt.legs[0].leg.to(), &counterparty);
        assert_eq!(receipt.legs[1].leg.from(), &counterparty);
        assert_eq!(receipt.legs[1].leg.to(), &new_account);
        let details = receipt.fx_corridor.as_ref().expect("FX receipt details");
        assert_eq!(details.source_account, new_account);
        assert_eq!(details.source_sink, new_account);
        assert_eq!(details.destination_reserve, new_account);
        assert_eq!(details.recipient, new_account);
    }

    #[test]
    fn rekey_public_lane_validators_ignores_mismatched_rows() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-rekey-public-lane-validators"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        let old_key = checked_keypair();
        let old_account = new_account_id(&old_key);
        let new_key = checked_keypair();
        let new_account = new_account_id(&new_key);
        let valid_lane = iroha_data_model::nexus::LaneId::new(8);
        let malformed_lane = iroha_data_model::nexus::LaneId::new(9);
        let active = iroha_data_model::nexus::PublicLaneValidatorStatus::Active;
        let reward_asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("multisig", "universal").expect("reward asset domain"),
            "reward".parse().expect("reward asset name"),
        );
        let old_reward_asset = iroha_data_model::asset::AssetId::new(
            reward_asset_definition.clone(),
            old_account.clone(),
        );
        let new_reward_asset = iroha_data_model::asset::AssetId::new(
            reward_asset_definition.clone(),
            new_account.clone(),
        );

        state_transaction.world.public_lane_validators.insert(
            (valid_lane, old_account.clone()),
            iroha_data_model::nexus::PublicLaneValidatorRecord {
                lane_id: valid_lane,
                validator: old_account.clone(),
                peer_id: iroha_data_model::peer::PeerId::from(
                    old_account.expect_single_signatory().clone(),
                ),
                stake_account: old_account.clone(),
                total_stake: iroha_primitives::numeric::Quantity::from(1_u32),
                self_stake: iroha_primitives::numeric::Quantity::from(1_u32),
                metadata: Metadata::default(),
                status: active.clone(),
                activation_epoch: Some(1),
                activation_height: Some(1),
                last_reward_epoch: None,
            },
        );
        state_transaction.world.public_lane_validators.insert(
            (malformed_lane, new_account.clone()),
            iroha_data_model::nexus::PublicLaneValidatorRecord {
                lane_id: malformed_lane,
                validator: old_account.clone(),
                peer_id: iroha_data_model::peer::PeerId::from(
                    old_account.expect_single_signatory().clone(),
                ),
                stake_account: old_account.clone(),
                total_stake: iroha_primitives::numeric::Quantity::from(2_u32),
                self_stake: iroha_primitives::numeric::Quantity::from(2_u32),
                metadata: Metadata::default(),
                status: active,
                activation_epoch: Some(1),
                activation_height: Some(1),
                last_reward_epoch: None,
            },
        );
        state_transaction.world.public_lane_stake_shares.insert(
            (valid_lane, old_account.clone(), old_account.clone()),
            iroha_data_model::nexus::PublicLaneStakeShare {
                lane_id: valid_lane,
                validator: old_account.clone(),
                staker: old_account.clone(),
                bonded: iroha_primitives::numeric::Quantity::from(3_u32),
                pending_unbonds: BTreeMap::new(),
                metadata: Metadata::default(),
            },
        );
        state_transaction.world.public_lane_stake_shares.insert(
            (malformed_lane, old_account.clone(), old_account.clone()),
            iroha_data_model::nexus::PublicLaneStakeShare {
                lane_id: malformed_lane,
                validator: new_account.clone(),
                staker: old_account.clone(),
                bonded: iroha_primitives::numeric::Quantity::from(4_u32),
                pending_unbonds: BTreeMap::new(),
                metadata: Metadata::default(),
            },
        );
        state_transaction.world.public_lane_rewards.insert(
            (valid_lane, 2),
            iroha_data_model::nexus::PublicLaneRewardRecord {
                lane_id: valid_lane,
                epoch: 2,
                asset: old_reward_asset.clone(),
                total_reward: iroha_primitives::numeric::Quantity::from(5_u32),
                shares: vec![iroha_data_model::nexus::PublicLaneRewardShare {
                    account: old_account.clone(),
                    role: iroha_data_model::nexus::PublicLaneRewardRole::Validator,
                    amount: iroha_primitives::numeric::Quantity::from(5_u32),
                }],
                metadata: Metadata::default(),
            },
        );
        state_transaction.world.public_lane_rewards.insert(
            (malformed_lane, 3),
            iroha_data_model::nexus::PublicLaneRewardRecord {
                lane_id: malformed_lane,
                epoch: 4,
                asset: old_reward_asset.clone(),
                total_reward: iroha_primitives::numeric::Quantity::from(6_u32),
                shares: vec![iroha_data_model::nexus::PublicLaneRewardShare {
                    account: old_account.clone(),
                    role: iroha_data_model::nexus::PublicLaneRewardRole::Validator,
                    amount: iroha_primitives::numeric::Quantity::from(6_u32),
                }],
                metadata: Metadata::default(),
            },
        );

        replace_account_id_in_public_lane(&mut state_transaction, &old_account, &new_account);

        assert!(
            state_transaction
                .world
                .public_lane_validators
                .get(&(valid_lane, old_account.clone()))
                .is_none(),
            "matching validator row should move away from the old key"
        );
        let moved = state_transaction
            .world
            .public_lane_validators
            .get(&(valid_lane, new_account.clone()))
            .expect("matching validator row should move to the new key");
        assert_eq!(moved.validator, new_account);
        assert_eq!(moved.stake_account, new_account);

        let malformed = state_transaction
            .world
            .public_lane_validators
            .get(&(malformed_lane, moved.validator.clone()))
            .expect("malformed row keyed by the new account should remain present");
        assert_eq!(
            malformed.validator, old_account,
            "mismatched row must not be repaired into a live matching validator"
        );
        assert_eq!(malformed.stake_account, old_account);

        assert!(
            state_transaction
                .world
                .public_lane_stake_shares
                .get(&(valid_lane, old_account.clone(), old_account.clone()))
                .is_none(),
            "matching stake-share row should move away from the old key"
        );
        let moved_share = state_transaction
            .world
            .public_lane_stake_shares
            .get(&(valid_lane, new_account.clone(), new_account.clone()))
            .expect("matching stake-share row should move to the new key");
        assert_eq!(moved_share.validator, new_account);
        assert_eq!(moved_share.staker, new_account);
        let malformed_share = state_transaction
            .world
            .public_lane_stake_shares
            .get(&(malformed_lane, old_account.clone(), old_account.clone()))
            .expect("malformed stake-share row should remain on its old key");
        assert_eq!(
            malformed_share.validator, new_account,
            "mismatched stake-share validator must not be rewritten through key repair"
        );
        assert_eq!(malformed_share.staker, old_account);
        assert!(
            state_transaction
                .world
                .public_lane_stake_shares
                .get(&(malformed_lane, new_account.clone(), new_account.clone()))
                .is_none(),
            "malformed stake-share row must not be moved into a live matching key"
        );

        let moved_reward = state_transaction
            .world
            .public_lane_rewards
            .get(&(valid_lane, 2))
            .expect("matching reward row should remain present");
        assert_eq!(moved_reward.asset, new_reward_asset);
        assert_eq!(moved_reward.shares[0].account, new_account);
        let malformed_reward = state_transaction
            .world
            .public_lane_rewards
            .get(&(malformed_lane, 3))
            .expect("malformed reward row should remain present");
        assert_eq!(
            malformed_reward.asset, old_reward_asset,
            "mismatched reward row asset must not be rewritten"
        );
        assert_eq!(malformed_reward.shares[0].account, old_account);
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("default", "universal").unwrap();

        let old_key = checked_keypair();
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
        let (_, old_asset_value) =
            iroha_data_model::asset::Asset::new(old_asset_id.clone(), Quantity::from(5_u32))
                .into_key_value();
        state_transaction
            .world
            .assets
            .insert(old_asset_id.clone(), old_asset_value);
        state_transaction.world.track_asset_holder(&old_asset_id);

        let new_key = checked_keypair();
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
        let source_domain: iroha_data_model::domain::DomainId =
            DomainId::try_new("default", "universal").unwrap();
        let signer = new_account_id(&checked_keypair());
        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer.clone(), 1)]),
            quorum: NonZeroU16::new(1).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let register = MultisigRegister::with_account(
            new_account_id(&checked_keypair()),
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
    fn multisig_metadata_cannot_reconstruct_missing_native_account_state() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-native-state-required"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        let domain_id = DomainId::try_new("bsp", "cbsi").expect("parse FI domain");
        let owner_id = new_account_id(&checked_keypair());
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer_id = new_account_id(&checked_keypair());
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
        execute_register(
            &mut state_transaction,
            &owner_id,
            MultisigRegister::with_account(
                new_account_id(&checked_keypair()),
                domain_id.clone(),
                spec.clone(),
            ),
        )
        .expect("register multisig");

        let registered_multisig_id =
            AccountId::new_multisig(multisig_policy_from_spec(&spec).expect("policy"));
        state_transaction
            .world
            .smart_contract_state
            .remove(multisig_account_state_key(&registered_multisig_id));
        let error = execute_propose(
            &mut state_transaction,
            &signer_id,
            &MultisigPropose::new(registered_multisig_id.clone(), Vec::new(), None),
        )
        .expect_err("metadata alone must not materialize native multisig state");
        assert!(matches!(
            error,
            ValidationFail::QueryFailed(QueryExecutionFail::NotFound)
        ));
        assert!(
            state_transaction
                .world
                .smart_contract_state
                .get(&multisig_account_state_key(&registered_multisig_id))
                .is_none(),
            "rejected proposal must not recreate native account state",
        );
    }

    #[test]
    fn multisig_metadata_must_match_native_account_state() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-metadata-native-state-consistency"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        let domain_id = DomainId::try_new("bsp", "cbsi").expect("parse FI domain");
        let owner_id = new_account_id(&checked_keypair());
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer_id = new_account_id(&checked_keypair());
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
        execute_register(
            &mut state_transaction,
            &owner_id,
            MultisigRegister::with_account(
                new_account_id(&checked_keypair()),
                domain_id.clone(),
                spec.clone(),
            ),
        )
        .expect("register multisig");

        let registered_multisig_id =
            AccountId::new_multisig(multisig_policy_from_spec(&spec).expect("policy"));
        let mut divergent_spec = spec.clone();
        divergent_spec.transaction_ttl_ms = NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS + 1).unwrap();
        state_transaction
            .world
            .accounts
            .get_mut(&registered_multisig_id)
            .expect("multisig account")
            .metadata
            .insert(spec_key(), Json::new(divergent_spec));

        let error = execute_propose(
            &mut state_transaction,
            &signer_id,
            &MultisigPropose::new(registered_multisig_id.clone(), Vec::new(), None),
        )
        .expect_err("spec metadata disagreement must fail closed");
        assert!(matches!(
            error,
            ValidationFail::QueryFailed(QueryExecutionFail::Conversion(message))
                if message.contains("multisig/spec metadata disagrees")
        ));

        let account = state_transaction
            .world
            .accounts
            .get_mut(&registered_multisig_id)
            .expect("multisig account");
        account.metadata.insert(spec_key(), Json::new(spec));
        account
            .metadata
            .insert(home_domain_key(), Json::new(None::<DomainId>));

        let error = execute_propose(
            &mut state_transaction,
            &signer_id,
            &MultisigPropose::new(registered_multisig_id, Vec::new(), None),
        )
        .expect_err("home-domain metadata disagreement must fail closed");
        assert!(matches!(
            error,
            ValidationFail::QueryFailed(QueryExecutionFail::Conversion(message))
                if message.contains("home-domain metadata disagrees")
        ));
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

        let owner_id = new_account_id(&checked_keypair());
        Register::account(iroha_data_model::account::NewAccount::new(owner_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register domainless owner");

        let signer = new_account_id(&checked_keypair());
        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer.clone(), 1)]),
            quorum: NonZeroU16::new(1).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_seed = new_account_id(&checked_keypair());

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
        let shared_key = checked_keypair().public_key().clone();

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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("wonderland", "universal").unwrap();

        let owner_key = checked_keypair();
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

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("ttl", "universal").unwrap();

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);

        register_domain_with_name_lease(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            "domain registration",
        );

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
        let multisig_account_key = checked_keypair();
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("signatory", "universal").unwrap();

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);

        register_domain_with_name_lease(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            "domain registration",
        );
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("repairable", "universal").unwrap();

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
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

        register_domain_with_name_lease(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            "domain registration",
        );
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
        let error = execute_propose(&mut state_transaction, &signer1_id, &propose)
            .expect_err("controller data must not reconstruct missing authenticated state");
        assert!(matches!(
            error,
            ValidationFail::QueryFailed(QueryExecutionFail::NotFound)
        ));
        assert!(
            state_transaction
                .world
                .smart_contract_state
                .get(&multisig_account_state_key(&multisig_id))
                .is_none(),
            "failed reconstruction must leave native multisig state absent"
        );
        assert!(load_signatory_memberships(&state_transaction, &signer1_id).is_empty());
        assert!(load_signatory_memberships(&state_transaction, &signer2_id).is_empty());
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("signatory-index", "universal").unwrap();

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);

        register_domain_with_name_lease(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            "domain registration",
        );
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("signatory-rekey", "universal").unwrap();

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
        let signer3 = checked_keypair();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        let signer3_id = new_account_id(&signer3);

        register_domain_with_name_lease(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            "domain registration",
        );
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
            transaction::Executable,
            trigger::{
                Trigger,
                action::{Action, Repeats},
            },
        };
        use ivm::{
            KotodamaCompiler,
            kotodama::compiler::{CompilerMode, CompilerOptions},
        };

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

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        let owner_keypair = checked_keypair();
        let owner_id = new_account_id(&owner_keypair);

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
        let multisig_id = new_account_id(&checked_keypair());
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
        Grant::account_permission(
            Permission::new("Admin".to_owned(), Json::new(())),
            multisig_id.clone(),
        )
        .execute(&owner_id, &mut state_transaction)
        .expect("grant trigger entrypoint permission to the multisig authority");

        let (program, manifest) =
            KotodamaCompiler::new_with_options(CompilerOptions {
                mode: CompilerMode::Production,
                ..CompilerOptions::default()
            })
            .compile_source_with_manifest(
                r#"
seiyaku TriggerDispatch {
  kotoage fn main() authorize("Admin") {
    ledger::account::set_detail(account: context::authority(), key: Name::parse("entrypoint"), value: Json::parse("1"));
  }

  kotoage fn alternate() authorize("Admin") {
    ledger::account::set_detail(account: context::authority(), key: Name::parse("entrypoint"), value: Json::parse("2"));
  }
}
"#,
            )
            .expect("compile trigger dispatch contract");
        let (bytecode, contract_address) = install_trigger_contract(
            &mut state_transaction,
            &owner_id,
            &owner_keypair,
            program,
            manifest,
            6_061,
        );

        let trigger_id: iroha_data_model::trigger::TriggerId = "contract_dispatch".parse().unwrap();
        let mut trigger_metadata = Metadata::default();
        trigger_metadata.insert(
            Name::from_str("contract_entrypoint").expect("static metadata key"),
            Json::new("alternate"),
        );
        trigger_metadata.insert(
            Name::from_str("contract_address").expect("static metadata key"),
            Json::new(contract_address.to_string()),
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("signatory-approve", "universal").unwrap();

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);

        register_domain_with_name_lease(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            "domain registration",
        );
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

        let pending_entrypoint_hash = Hash::prehashed([0xa4; Hash::LENGTH]);
        state_transaction.tx_call_hash = Some(pending_entrypoint_hash);
        let pending_approve = MultisigApprove::new(multisig_id.clone(), instructions_hash);
        execute_approve(&mut state_transaction, &signer1_id, &pending_approve)
            .expect("duplicate subject approval should remain below quorum");
        let pending_outcome_key = multisig_approval_outcome_state_key(
            *pending_entrypoint_hash.as_ref(),
            &multisig_id,
            &instructions_hash,
        );
        let pending_outcome_bytes = state_transaction
            .world
            .smart_contract_state
            .get(&pending_outcome_key)
            .expect("successful pre-quorum approval should leave an exact outcome");
        let pending_outcome =
            norito::decode_from_bytes::<MultisigApprovalOutcomeV1>(pending_outcome_bytes)
                .expect("pre-quorum approval outcome should decode");
        assert_eq!(
            pending_outcome.status,
            MultisigApprovalOutcomeStatusV1::NotExecuted
        );
        assert_eq!(pending_outcome.entrypoint_account_id, multisig_id);
        assert_eq!(pending_outcome.resolved_multisig_account_id, multisig_id);

        let terminal_entrypoint_hash = Hash::prehashed([0xa5; Hash::LENGTH]);
        state_transaction.tx_call_hash = Some(terminal_entrypoint_hash);
        let approve = MultisigApprove::new(multisig_id.clone(), instructions_hash);
        execute_approve(&mut state_transaction, &signer2_id, &approve)
            .expect("signatory approve without roles");

        let execution_key = multisig_proposal_terminal_execution_state_key(
            *terminal_entrypoint_hash.as_ref(),
            &multisig_id,
            &instructions_hash,
        );
        let execution_bytes = state_transaction
            .world
            .smart_contract_state
            .get(&execution_key)
            .expect("finalized proposal should leave transaction-bound terminal state")
            .clone();
        let execution_state =
            norito::decode_from_bytes::<MultisigProposalTerminalExecutionStateV1>(&execution_bytes)
                .expect("transaction-bound terminal state should decode");
        assert_eq!(execution_state.terminal_block_height, 1);
        assert_eq!(execution_state.entrypoint_account_id, multisig_id);
        assert_eq!(
            execution_state.terminal_entrypoint_hash,
            *terminal_entrypoint_hash.as_ref()
        );
        let executed_outcome_key = multisig_approval_outcome_state_key(
            *terminal_entrypoint_hash.as_ref(),
            &multisig_id,
            &instructions_hash,
        );
        let executed_outcome_bytes = state_transaction
            .world
            .smart_contract_state
            .get(&executed_outcome_key)
            .expect("quorum approval should leave an exact executed outcome");
        let executed_outcome =
            norito::decode_from_bytes::<MultisigApprovalOutcomeV1>(executed_outcome_bytes)
                .expect("executed approval outcome should decode");
        assert_eq!(
            executed_outcome.status,
            MultisigApprovalOutcomeStatusV1::Executed
        );
        assert_eq!(executed_outcome.entrypoint_account_id, multisig_id);
        assert_eq!(executed_outcome.resolved_multisig_account_id, multisig_id);

        let mut conflicting_state = execution_state.terminal;
        conflicting_state.terminal_at_ms = conflicting_state.terminal_at_ms.saturating_add(1);
        let err = store_multisig_proposal_terminal_execution_state(
            &mut state_transaction,
            &conflicting_state,
            &multisig_id,
        )
        .expect_err("transaction-bound terminal state must be append-only");
        assert!(
            matches!(err, ValidationFail::InternalError(message) if message.contains("conflicting immutable multisig terminal execution state"))
        );
        assert_eq!(
            state_transaction
                .world
                .smart_contract_state
                .get(&execution_key),
            Some(&execution_bytes),
            "conflicting write must not alter transaction-bound terminal state"
        );
    }

    #[test]
    fn relayed_multisig_execution_leaves_only_transaction_bound_terminal_state() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-relayed-terminal-state"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        let relay_account = new_account_id(&checked_keypair());
        let relay_instructions = vec![InstructionBox::from(Log::new(
            Level::INFO,
            "relay execution".to_owned(),
        ))];
        let relay_instructions_hash = HashOf::new(&relay_instructions);
        let relay_state = MultisigProposalState::new(
            relay_account.clone(),
            relay_instructions_hash,
            relay_instructions,
            1,
            u64::MAX,
            BTreeSet::from([relay_account.clone()]),
            Some(true),
        );
        let terminal_entrypoint_hash = Hash::prehashed([0xb7; Hash::LENGTH]);
        state_transaction.tx_call_hash = Some(terminal_entrypoint_hash);

        maybe_store_relayed_proposal_execution_state(
            &mut state_transaction,
            &relay_state,
            &relay_account,
        )
        .expect("relayed execution should persist immutable evidence");

        let execution_key = multisig_proposal_terminal_execution_state_key(
            *terminal_entrypoint_hash.as_ref(),
            &relay_account,
            &relay_instructions_hash,
        );
        let execution_bytes = state_transaction
            .world
            .smart_contract_state
            .get(&execution_key)
            .expect("relayed execution should leave transaction-bound terminal state");
        let execution_state =
            norito::decode_from_bytes::<MultisigProposalTerminalExecutionStateV1>(execution_bytes)
                .expect("relayed terminal execution state should decode");
        assert_eq!(
            execution_state.terminal.status,
            MultisigProposalTerminalStatus::Finalized
        );
        assert_eq!(execution_state.terminal.proposal.is_relayed, Some(true));
        assert_eq!(execution_state.entrypoint_account_id, relay_account);
        assert_eq!(execution_state.terminal_block_height, 1);
        assert_eq!(
            execution_state.terminal_entrypoint_hash,
            *terminal_entrypoint_hash.as_ref()
        );
        assert!(
            state_transaction
                .world
                .smart_contract_state
                .get(&multisig_proposal_terminal_state_key(
                    &relay_account,
                    &relay_instructions_hash,
                ))
                .is_none(),
            "relayed lifecycle state remains active and must not create the mutable terminal key"
        );
    }

    #[test]
    fn terminal_state_rekey_does_not_fabricate_transaction_bound_execution_evidence() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-terminal-rekey-evidence"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        let old_account = new_account_id(&checked_keypair());
        let new_account = new_account_id(&checked_keypair());
        let instructions = vec![InstructionBox::from(Log::new(
            Level::INFO,
            "historical execution".to_owned(),
        ))];
        let instructions_hash = HashOf::new(&instructions);
        let terminal_state = MultisigProposalTerminalState::new(
            old_account.clone(),
            instructions_hash,
            MultisigProposalValue::new(
                instructions,
                1,
                u64::MAX,
                BTreeSet::from([old_account.clone()]),
                None,
            ),
            MultisigProposalTerminalStatus::Finalized,
            2,
        );
        store_multisig_proposal_terminal_state(&mut state_transaction, &terminal_state)
            .expect("historical terminal state should store");

        let rekey_entrypoint_hash = Hash::prehashed([0xc9; Hash::LENGTH]);
        state_transaction.tx_call_hash = Some(rekey_entrypoint_hash);
        move_multisig_proposals(&mut state_transaction, &old_account, &new_account)
            .expect("terminal state should move during rekey");

        let moved_bytes = state_transaction
            .world
            .smart_contract_state
            .get(&multisig_proposal_terminal_state_key(
                &new_account,
                &instructions_hash,
            ))
            .expect("rekeyed terminal state should exist");
        let moved_state = norito::decode_from_bytes::<MultisigProposalTerminalState>(moved_bytes)
            .expect("rekeyed terminal state should decode");
        assert_eq!(moved_state.multisig_account_id, new_account);
        assert!(
            state_transaction
                .world
                .smart_contract_state
                .get(&multisig_proposal_terminal_execution_state_key(
                    *rekey_entrypoint_hash.as_ref(),
                    &new_account,
                    &instructions_hash,
                ))
                .is_none(),
            "rekeying historical lifecycle state must not create execution evidence"
        );
    }

    #[test]
    fn multisig_approve_executes_staged_mint_like_trigger_with_json_args() {
        use iroha_data_model::{
            events::execute_trigger::ExecuteTriggerEventFilter,
            transaction::Executable,
            trigger::{
                Trigger,
                action::{Action, Repeats},
            },
        };

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-staged-mint-json-args"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: DomainId = DomainId::try_new("staged", "universal").unwrap();

        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
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
        Grant::account_permission(
            Permission::new("staged_mint_request_run".into(), Json::new(())),
            multisig_id.clone(),
        )
        .execute(&signer1_id, &mut state_transaction)
        .expect("grant staged mint permission");

        let src = format!(
            r#"
            seiyaku StagedMintRequest {{
              // Runtime trigger authority: {multisig_id}
              error enum StagedMintError {{
                DuplicateRequest = 1,
                UnsupportedAction = 2,
                UnsupportedAssetDefinition = 3,
                DestinationAccountMismatch = 4,
                InvalidAmount = 5,
                MissingOrInvalidField = 6,
              }}

              state StateMap<Name, bytes> Requests_requested_by_actor;
              state StateMap<Name, AccountId> ToAccount;
              state StateMap<Name, quantity> Amount;
              state StateMap<Name, int> ProposalStatus;
              state StateMap<Name, int> CreatedAtMs;
              state StateMap<Name, int> ExpiresAtMs;

              fn run_impl(Json ev) -> Option<bool> {{
                let request_id = ev.get_name(Name::parse("request_id"))?;
                require(!ProposalStatus.contains(request_id), StagedMintError::DuplicateRequest);
                let action = ev.get_name(Name::parse("action"))?;
                require(action == Name::parse("create"), StagedMintError::UnsupportedAction);
                let asset_id = ev.get_asset_definition_id(Name::parse("asset_id"))?;
                let expected_asset = AssetDefinitionId::parse("66owaQmAQMuHxPzxUN3bqZ6FJfDa");
                require(asset_id == expected_asset, StagedMintError::UnsupportedAssetDefinition);
                let to_account_id = ev.get_account_id(Name::parse("to_account_id"))?;
                require(
                  to_account_id == context::authority(),
                  StagedMintError::DestinationAccountMismatch,
                );
                let amount = ev.get_quantity(Name::parse("amount"))?;
                let requested_by_actor = ev.get_blob_hex(Name::parse("requested_by_actor_hex"))?;
                let created_at_ms = ev.get_int(Name::parse("created_at_ms"))?;
                let expires_at_ms = ev.get_int(Name::parse("expires_at_ms"))?;
                require(amount > 0, StagedMintError::InvalidAmount);

                Requests_requested_by_actor[request_id] = requested_by_actor;
                ToAccount[request_id] = to_account_id;
                Amount[request_id] = amount;
                ProposalStatus[request_id] = 1;
                CreatedAtMs[request_id] = created_at_ms;
                ExpiresAtMs[request_id] = expires_at_ms;
                Option::some(true)
              }}

              kotoage fn run(Json ev) authorize("staged_mint_request_run") {{
                require(run_impl(ev).is_some(), StagedMintError::MissingOrInvalidField);
              }}
            }}
            "#,
            multisig_id = multisig_id,
        );
        let (program, manifest) = ivm::KotodamaCompiler::new()
            .compile_source_with_manifest(&src)
            .expect("compile staged mint-like contract");
        let (bytecode, contract_address) = install_trigger_contract(
            &mut state_transaction,
            &signer1_id,
            &signer1,
            program,
            manifest,
            6_455,
        );
        let trigger_id: iroha_data_model::trigger::TriggerId = "staged_mint_like".parse().unwrap();
        let mut trigger_metadata = Metadata::default();
        trigger_metadata.insert(
            Name::from_str("contract_entrypoint").expect("static metadata key"),
            Json::new("run"),
        );
        trigger_metadata.insert(
            Name::from_str("contract_address").expect("static metadata key"),
            Json::new(contract_address.to_string()),
        );
        let trigger = Trigger::new(
            trigger_id.clone(),
            Action::new(
                Executable::Ivm(bytecode),
                Repeats::Indefinitely,
                multisig_id.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id.clone())
                    .under_authority(multisig_id.clone()),
            )
            .with_metadata(trigger_metadata),
        );
        Register::trigger(trigger)
            .execute(&multisig_id, &mut state_transaction)
            .expect("register event-argument-aware staged mint trigger");

        let args_json = format!(
            r#"{{
                "ev": {{
                    "action":"create",
                    "request_id":"mrtest",
                    "asset_id":"66owaQmAQMuHxPzxUN3bqZ6FJfDa",
                    "to_account_id":"{multisig_id}",
                    "amount":"111",
                    "requested_by_actor_hex":"0x7b226163746f72223a226f70657261746f7231227d",
                    "created_at_ms":1779225455574,
                    "expires_at_ms":1779311855574
                }}
            }}"#,
            multisig_id = multisig_id,
        );
        let instructions = vec![InstructionBox::from(
            ExecuteTrigger::new(trigger_id)
                .with_args(Json::from_raw_json(args_json).expect("valid event arguments JSON")),
        )];
        let instructions_hash = HashOf::new(&instructions);
        execute_propose(
            &mut state_transaction,
            &signer1_id,
            &MultisigPropose::new(multisig_id.clone(), instructions, None),
        )
        .expect("signatory propose staged mint");

        execute_approve(
            &mut state_transaction,
            &signer2_id,
            &MultisigApprove::new(multisig_id.clone(), instructions_hash),
        )
        .expect("signatory approve should execute staged mint trigger");

        assert!(
            proposal_state(&state_transaction, &multisig_id, &instructions_hash).is_err(),
            "finalized proposal should be pruned from active proposal state"
        );
        assert!(
            state_transaction
                .world
                .smart_contract_state
                .get(&multisig_proposal_terminal_state_key(
                    &multisig_id,
                    &instructions_hash,
                ))
                .is_some(),
            "finalized proposal should leave terminal proposal state"
        );

        let statuses = durable_state_values_under_contract_prefix(
            &state_transaction,
            &contract_address,
            "ProposalStatus",
        );
        assert_eq!(
            statuses.len(),
            1,
            "staged mint trigger should write one visible ProposalStatus entry"
        );
        assert_eq!(
            durable_int_value(&statuses[0]),
            1,
            "staged mint trigger should persist pending status"
        );
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

        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("retryable", "universal").unwrap();
        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
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

            register_domain_with_name_lease(
                &mut state_transaction,
                &signer1_id,
                &domain_id,
                "domain registration",
            );
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

            state_transaction.apply();
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

        let multisig_domain: iroha_data_model::domain::DomainId =
            DomainId::try_new("multisig-home", "universal").unwrap();
        let signer_domain: iroha_data_model::domain::DomainId =
            DomainId::try_new("signatory-remote", "universal").unwrap();

        let owner = checked_keypair();
        let signer1 = checked_keypair();
        let signer2 = checked_keypair();

        let owner_id = new_account_id(&owner);
        let signer1_remote = new_account_id(&signer1);
        let signer2_remote = new_account_id(&signer2);

        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &multisig_domain,
            "register multisig domain",
        );
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &signer_domain,
            "register signer domain",
        );

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
        let multisig_seed = new_account_id(&checked_keypair());
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

        let home_domain: iroha_data_model::domain::DomainId =
            DomainId::try_new("subject-home", "universal").unwrap();
        let alt_domain: iroha_data_model::domain::DomainId =
            DomainId::try_new("subject-alt", "universal").unwrap();

        let owner = checked_keypair();
        let shared_subject = checked_keypair();
        let signer_b = checked_keypair();
        let signer_c = checked_keypair();

        let owner_id = new_account_id(&owner);
        let shared_account = new_account_id(&shared_subject);
        let signer_b_id = new_account_id(&signer_b);
        let signer_c_id = new_account_id(&signer_c);

        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &home_domain,
            "register home domain",
        );
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &alt_domain,
            "register alt domain",
        );

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
        let multisig_seed = new_account_id(&checked_keypair());
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
                Err(ValidationFail::QueryFailed(QueryExecutionFail::NotFound))
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("signatory-single", "universal").unwrap();

        let (owner, leaf_a, leaf_b) = (checked_keypair(), checked_keypair(), checked_keypair());

        let owner_id = new_account_id(&owner);
        let first_leaf_account_id = new_account_id(&leaf_a);
        let second_leaf_account_id = new_account_id(&leaf_b);

        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
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
        let parent_key = checked_keypair();
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
    fn multisig_deferred_execution_reenters_active_executor_and_preserves_valid_flow() {
        let state = State::new_with_chain(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("multisig-deferred-active-executor"),
        );
        let block_header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id = DomainId::try_new("deferred", "universal").expect("domain id");
        let signer_id = new_account_id(&checked_keypair());

        register_domain_with_name_lease(
            &mut state_transaction,
            &signer_id,
            &domain_id,
            "register deferred-execution domain",
        );
        register_account_in_domain(
            &mut state_transaction,
            &signer_id,
            &domain_id,
            &signer_id,
            "register deferred-execution signer",
        );
        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer_id.clone(), 1)]),
            quorum: NonZeroU16::new(1).expect("non-zero quorum"),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).expect("non-zero ttl"),
        };
        let multisig_id = register_multisig_account(
            &mut state_transaction,
            &signer_id,
            &domain_id,
            &spec,
            "register deferred-execution multisig",
        );

        let metadata_key: Name = "deferred_executor_valid".parse().expect("metadata key");
        let metadata_value = Json::new("validated");
        let valid_instructions = vec![InstructionBox::from(SetKeyValue::account(
            multisig_id.clone(),
            metadata_key.clone(),
            metadata_value.clone(),
        ))];
        let valid_hash = HashOf::new(&valid_instructions);
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &signer_id,
                MultisigPropose::new(multisig_id.clone(), valid_instructions, None).into(),
            )
            .expect("propose valid deferred instruction");
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &signer_id,
                MultisigApprove::new(multisig_id.clone(), valid_hash).into(),
            )
            .expect("active initial executor should allow self-owned metadata mutation");
        assert_eq!(
            state_transaction
                .world
                .account(&multisig_id)
                .expect("multisig account")
                .metadata()
                .get(&metadata_key),
            Some(&metadata_value),
            "a valid deferred instruction should still execute"
        );

        let privileged_permission: Permission =
            iroha_executor_data_model::permission::executor::CanUpgradeExecutor.into();
        let privileged_instructions = vec![InstructionBox::from(Grant::account_permission(
            privileged_permission.clone(),
            signer_id.clone(),
        ))];
        let privileged_hash = HashOf::new(&privileged_instructions);
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &signer_id,
                MultisigPropose::new(multisig_id.clone(), privileged_instructions, None).into(),
            )
            .expect("propose privileged deferred instruction");
        let privileged_error = Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &signer_id,
                MultisigApprove::new(multisig_id.clone(), privileged_hash).into(),
            )
            .expect_err("multisig must not bypass genesis-only permission delegation");
        assert!(
            matches!(&privileged_error, ValidationFail::NotPermitted(_)),
            "unexpected privileged-instruction error: {privileged_error:?}"
        );
        assert!(
            !state_transaction
                .world
                .account_permissions_iter(&signer_id)
                .expect("signer permissions")
                .any(|permission| permission == &privileged_permission),
            "denied deferred grant must not mutate permissions"
        );

        let denied_instructions = vec![InstructionBox::from(Log::new(
            Level::INFO,
            "deferred instruction must reach the active executor".to_owned(),
        ))];
        let denied_hash = HashOf::new(&denied_instructions);
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &signer_id,
                MultisigPropose::new(multisig_id.clone(), denied_instructions, None).into(),
            )
            .expect("propose instruction before executor replacement");
        *state_transaction.world.executor.get_mut() =
            crate::executor::denying_executor_for_testing("deferred instruction denied");
        let denied_error = execute_approve(
            &mut state_transaction,
            &signer_id,
            &MultisigApprove::new(multisig_id, denied_hash),
        )
        .expect_err("final approval must submit stored instructions to the active executor");
        assert_eq!(
            denied_error,
            ValidationFail::NotPermitted("deferred instruction denied".to_owned())
        );
        assert!(
            state_transaction
                .multisig_deferred_execution_stack
                .is_empty(),
            "failed deferred execution must unwind its recursion guard"
        );
    }

    #[test]
    fn multisig_deferred_execution_guard_rejects_cycles_and_excessive_depth() {
        let account = new_account_id(&checked_keypair());
        let first_hash = HashOf::new(&vec![InstructionBox::from(Log::new(
            Level::INFO,
            "cycle".to_owned(),
        ))]);
        let first_id = (account.clone(), first_hash);
        let mut execution_stack = Vec::new();
        begin_multisig_deferred_execution(&mut execution_stack, &first_id)
            .expect("first proposal may execute");
        let cycle_error = begin_multisig_deferred_execution(&mut execution_stack, &first_id)
            .expect_err("an active proposal may not recursively execute itself");
        assert!(matches!(cycle_error, ValidationFail::NotPermitted(_)));
        finish_multisig_deferred_execution(&mut execution_stack, &first_id);

        let mut execution_ids = Vec::with_capacity(MAX_MULTISIG_DEFERRED_EXECUTION_DEPTH);
        for depth in 0..MAX_MULTISIG_DEFERRED_EXECUTION_DEPTH {
            let hash = HashOf::new(&vec![InstructionBox::from(Log::new(
                Level::INFO,
                format!("depth-{depth}"),
            ))]);
            let execution_id = (account.clone(), hash);
            begin_multisig_deferred_execution(&mut execution_stack, &execution_id)
                .expect("proposal within the deferred execution depth bound");
            execution_ids.push(execution_id);
        }
        let beyond_limit = (
            account,
            HashOf::new(&vec![InstructionBox::from(Log::new(
                Level::INFO,
                "beyond-limit".to_owned(),
            ))]),
        );
        assert_eq!(
            begin_multisig_deferred_execution(&mut execution_stack, &beyond_limit),
            Err(ValidationFail::TooComplex)
        );
        for execution_id in execution_ids.iter().rev() {
            finish_multisig_deferred_execution(&mut execution_stack, execution_id);
        }
        assert!(execution_stack.is_empty());
    }

    #[test]
    fn multisig_expiry_traversal_rejects_cycles_and_excessive_depth() {
        let member_key = checked_keypair();
        let member =
            MultisigMember::new(member_key.public_key().clone(), 1).expect("multisig member");
        let policy = MultisigPolicy::new(1, vec![member]).expect("multisig policy");
        let multisig_id = AccountId::new_multisig(policy);
        let owner_id = new_account_id(&checked_keypair());
        let world = World::with([], [Account::new(multisig_id.clone()).build(&owner_id)], []);
        let state = State::new_with_chain(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("multisig-expiry-traversal-guard"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        let cycle_hash = HashOf::new(&Vec::<InstructionBox>::new());
        let cycle_instruction = MultisigApprove::new(multisig_id.clone(), cycle_hash);
        store_multisig_proposal_state(
            &mut state_transaction,
            &MultisigProposalState::new(
                multisig_id.clone(),
                cycle_hash,
                vec![cycle_instruction.into()],
                0,
                0,
                BTreeSet::new(),
                None,
            ),
        )
        .expect("store cyclic proposal state");
        let cycle_error = prune_expired(
            &mut state_transaction,
            &multisig_id,
            &cycle_hash,
            &multisig_id,
        )
        .expect_err("cyclic expiry traversal must fail closed");
        assert!(matches!(cycle_error, ValidationFail::NotPermitted(_)));

        let chain_hashes: Vec<_> = (0..=MAX_MULTISIG_DEFERRED_EXECUTION_DEPTH)
            .map(|depth| {
                HashOf::new(&vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    format!("expiry-depth-{depth}"),
                ))])
            })
            .collect();
        for pair in chain_hashes.windows(2) {
            store_multisig_proposal_state(
                &mut state_transaction,
                &MultisigProposalState::new(
                    multisig_id.clone(),
                    pair[0],
                    vec![MultisigApprove::new(multisig_id.clone(), pair[1]).into()],
                    0,
                    0,
                    BTreeSet::new(),
                    None,
                ),
            )
            .expect("store deep proposal state");
        }
        let depth_error = prune_expired(
            &mut state_transaction,
            &multisig_id,
            &chain_hashes[0],
            &multisig_id,
        )
        .expect_err("expiry traversal beyond the deterministic depth bound must fail");
        assert_eq!(depth_error, ValidationFail::TooComplex);
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("missing", "universal").unwrap();

        let owner_key = checked_keypair();
        let owner_id = new_account_id(&owner_key);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
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
            ValidationFail::QueryFailed(QueryExecutionFail::NotFound) => {}
            other => panic!("unexpected error for missing multisig spec: {other:?}"),
        }
    }

    #[test]
    fn multisig_role_for_large_policy_uses_hash_suffix() {
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("weights", "universal").unwrap();
        let member_count = (u8::MAX as usize) + 1;
        let mut members = Vec::with_capacity(member_count);
        for _ in 0..member_count {
            let key = checked_keypair();
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("cancel", "universal").unwrap();

        let owner_key = checked_keypair();
        let owner_id = new_account_id(&owner_key);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1_key = checked_keypair();
        let signer1_id = new_account_id(&signer1_key);
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &signer1_id,
            "register signer1",
        );
        let signer2_key = checked_keypair();
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("weights", "universal").unwrap();

        let owner_key = checked_keypair();
        let owner_id = new_account_id(&owner_key);
        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
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
            let signer_key = checked_keypair();
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("replace", "universal").unwrap();
        let owner_key = checked_keypair();
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

        register_domain_with_name_lease(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            "domain registration",
        );
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

        let member1 = checked_keypair();
        let member2 = checked_keypair();
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("single", "universal").unwrap();
        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
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

        let replacement_key = checked_keypair();
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
        let domain_id: iroha_data_model::domain::DomainId =
            DomainId::try_new("repoint", "universal").unwrap();
        let signer1 = checked_keypair();
        let signer2 = checked_keypair();
        let signer3 = checked_keypair();
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
