//! Built-in handling for multisig instructions without requiring an executor upgrade.

use std::{collections::BTreeSet, str::FromStr};

use iroha_crypto::HashOf;
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
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
        if has_role(state_transaction, &account, &signatory_role_id).map_err(map_validation_fail)? {
            Revoke::account_role(signatory_role_id, account.clone())
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
    let suffix = match account.controller() {
        iroha_data_model::account::AccountController::Single(_) => {
            account.signatory().to_string()
        }
        iroha_data_model::account::AccountController::Multisig(_) => account.to_string(),
    };
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
    // TODO: migrate multisig accounts to a controller derived from spec once account-based members
    // are supported in the account controller surface.
    let _ = (state_transaction, spec);
    Ok(account.clone())
}

fn execute_register(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: MultisigRegister,
) -> Result<(), ValidationFail> {
    let spec = instruction.spec;
    let multisig_account_id = instruction.account;
    let domain_id = validate_registration(state_transaction, &multisig_account_id, &spec)?;
    let domain_owner = domain_owner(state_transaction, &domain_id)?;

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

    configure_roles(
        state_transaction,
        &domain_owner,
        &multisig_account_id,
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
    let is_signatory = multisig_spec.signatories.contains_key(&proposer);
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
    for signatory in multisig_spec.signatories.keys() {
        if is_multisig(state_transaction, signatory)? {
            deploy_relayer(
                state_transaction,
                signatory,
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
    let is_signatory = spec.signatories.contains_key(&approver);
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

    proposal_value.approvals.insert(approver);
    SetKeyValue::account(
        multisig_account.clone(),
        proposal_key(&instructions_hash),
        proposal_value.clone(),
    )
    .execute(&multisig_account, state_transaction)
    .map_err(ValidationFail::InstructionFailed)?;

    let approved_weight: u32 = spec
        .signatories
        .iter()
        .filter(|(id, _)| proposal_value.approvals.contains(*id))
        .map(|(_, weight)| u32::from(*weight))
        .sum();
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

    for signatory in spec.signatories.keys() {
        if is_multisig(state_transaction, signatory)? {
            deploy_relayer(
                state_transaction,
                signatory,
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

    for signatory in spec.signatories.keys() {
        let relay_hash = {
            let relay = MultisigApprove::new(multisig_account.clone(), *instructions_hash);
            HashOf::new(&vec![InstructionBox::from(relay)])
        };
        if is_multisig(state_transaction, signatory)? {
            prune_down(state_transaction, signatory, &relay_hash)?;
        }
    }

    Ok(())
}

fn validate_registration(
    state_transaction: &mut StateTransaction<'_, '_>,
    multisig_account: &AccountId,
    spec: &MultisigSpec,
) -> Result<iroha_data_model::domain::DomainId, ValidationFail> {
    let domain_id = signatory_domain(spec)?;
    if multisig_account.domain() != &domain_id {
        return Err(ValidationFail::NotPermitted(format!(
            "multisig account `{multisig_account}` must belong to domain `{domain_id}`"
        )));
    }
    ensure_quorum_reachable(spec)?;
    ensure_signatories_exist(state_transaction, spec)?;
    ensure_multisig_graph_is_acyclic(spec.signatories.keys().cloned(), state_transaction)?;
    Ok(domain_id)
}

fn signatory_domain(
    spec: &MultisigSpec,
) -> Result<iroha_data_model::domain::DomainId, ValidationFail> {
    let mut signatories = spec.signatories.keys();
    let Some(first) = signatories.next() else {
        return Err(ValidationFail::NotPermitted(
            "multisig spec must include at least one signatory".to_owned(),
        ));
    };
    let domain = first.domain().clone();

    for account in signatories {
        if account.domain() != &domain {
            return Err(ValidationFail::NotPermitted(format!(
                "multisig signatory `{account}` must belong to domain `{domain}`"
            )));
        }
    }

    Ok(domain)
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
        state_transaction
            .world
            .account(account)
            .map_err(map_find_error)?;
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
            Ok(spec)
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
    let signatories: Vec<AccountId> = spec.signatories.keys().cloned().collect();

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
    raw.try_into_any_norito().map_err(|err| {
        InstructionExecutionError::Conversion(format!("multisig spec metadata malformed:\n{err}"))
    })
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
        multisig_id
    }

    fn register_multisig_role(
        state_transaction: &mut StateTransaction<'_, '_>,
        owner_id: &AccountId,
        role_id: &RoleId,
        account_id: &AccountId,
    ) {
        Register::role(Role::new(role_id.clone(), owner_id.clone()))
            .execute(owner_id, state_transaction)
            .expect("register multisig role");
        Grant::account_role(role_id.clone(), account_id.clone())
            .execute(owner_id, state_transaction)
            .expect("grant multisig role");
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
        let domain_id: iroha_data_model::domain::DomainId = "sbp".parse().unwrap();

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
        let account_after_register = state_transaction
            .world
            .account(&multisig_id)
            .expect("multisig account registered");
        assert!(
            account_after_register.metadata().get(&spec_key).is_some(),
            "multisig spec metadata must be stored on registration"
        );
        let stored_spec =
            multisig_spec(&state_transaction, &multisig_id).expect("spec must decode");
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

        let updated =
            multisig_spec(&state_transaction, &multisig_id).expect("spec must decode after add");
        assert!(
            updated.signatories.contains_key(&signer2_id),
            "added signatory must appear in spec"
        );
        let multisig_role = multisig_role_for(&multisig_id);
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
                .account_roles_iter(&multisig_id)
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

        let updated =
            multisig_spec(&state_transaction, &multisig_id).expect("spec must decode after remove");
        assert!(
            !updated.signatories.contains_key(&signer2_id),
            "removed signatory must be absent from spec"
        );
        let multisig_role = multisig_role_for(&multisig_id);
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
                .account_roles_iter(&multisig_id)
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

        let updated = multisig_spec(&state_transaction, &multisig_id)
            .expect("spec must decode after set quorum");
        assert_eq!(updated.quorum, new_quorum, "quorum update should persist");
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

        let override_ttl =
            NonZeroU64::new(spec.transaction_ttl_ms.get().saturating_add(1)).unwrap();
        let propose = MultisigPropose::new(multisig_id.clone(), Vec::new(), Some(override_ttl));

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
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &signer1_id,
                InstructionBox::from(propose),
            )
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
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &signer1_id,
                InstructionBox::from(propose),
            )
            .expect("signatory propose");

        let approve = MultisigApprove::new(multisig_id.clone(), instructions_hash);
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &signer2_id,
                InstructionBox::from(approve),
            )
            .expect("signatory approve without roles");
    }

    #[test]
    fn relayer_ttl_is_capped_by_nested_spec() {
        let state = State::new_with_chain(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("multisig-ttl-cap-chain"),
        );
        let now_ms = 1_000;
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, now_ms, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "ttlcap".parse().unwrap();

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

        let child_ttl_ms: u64 = 2_000;
        let child_spec = MultisigSpec {
            signatories: BTreeMap::from([
                (first_leaf_account_id.clone(), 1),
                (second_leaf_account_id.clone(), 1),
            ]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(child_ttl_ms).unwrap(),
        };
        let child_id = register_multisig_account(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &child_spec,
            "register child multisig account",
        );

        let parent_ttl_ms: u64 = 10_000;
        let parent_spec = MultisigSpec {
            signatories: BTreeMap::from([(owner_id.clone(), 1), (child_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(parent_ttl_ms).unwrap(),
        };
        let parent_id = register_multisig_account(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &parent_spec,
            "register parent multisig account",
        );

        let child_role = multisig_role_for(&child_id);
        let parent_role = multisig_role_for(&parent_id);
        register_multisig_role(&mut state_transaction, &owner_id, &child_role, &child_id);
        register_multisig_role(&mut state_transaction, &owner_id, &parent_role, &parent_id);
        let _ = Grant::account_role(parent_role.clone(), owner_id.clone())
            .execute(&owner_id, &mut state_transaction);

        let instructions: Vec<InstructionBox> = Vec::new();
        let instructions_hash = HashOf::new(&instructions);
        let proposal = MultisigPropose::new(parent_id.clone(), instructions, None);
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &owner_id,
                InstructionBox::from(proposal),
            )
            .expect("parent multisig propose");

        let parent_value = proposal_value(&state_transaction, &parent_id, &instructions_hash)
            .expect("parent value");
        assert_eq!(
            parent_value.expires_at_ms,
            now_ms + parent_ttl_ms,
            "parent TTL should remain unchanged"
        );

        let relay = MultisigApprove::new(parent_id.clone(), instructions_hash);
        let relay_hash = HashOf::new(&vec![InstructionBox::from(relay)]);
        let child_value =
            proposal_value(&state_transaction, &child_id, &relay_hash).expect("child value");
        assert_eq!(
            child_value.expires_at_ms,
            now_ms + child_ttl_ms,
            "nested relayer TTL must be capped by the child multisig policy"
        );
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
