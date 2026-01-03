//! Validation and execution logic of instructions for multisig accounts

use std::collections::BTreeSet;

use iroha_smart_contract::data_model::{
    prelude::{FindAccounts, Grant, Register},
    query::prelude::{FindDomains, FindRoles, FindRolesByAccountId},
    role::{Role, RoleId},
};

use super::*;
use crate::data_model::{
    domain::DomainId, isi::error::InstructionExecutionError, metadata::Metadata,
};

impl VisitExecute for MultisigRegister {
    fn visit<V: Execute + Visit + ?Sized>(&self, executor: &mut V) {
        if let Err(err) = validate_registration(&self.account, &self.spec, executor) {
            deny!(executor, err);
        }
    }

    fn execute<V: Execute + Visit + ?Sized>(self, executor: &mut V) -> Result<(), ValidationFail> {
        let spec = self.spec;
        let multisig_account_id = self.account;
        let domain_id = validate_registration(&multisig_account_id, &spec, executor)?;
        let domain_owner = domain_owner(&domain_id, executor)?;

        if account_exists(&multisig_account_id, executor)? {
            return Err(ValidationFail::NotPermitted(format!(
                "multisig account `{multisig_account_id}` already exists"
            )));
        }

        let mut metadata = Metadata::default();
        metadata.insert(spec_key(), Json::new(spec.clone()));

        let register_account =
            Register::account(Account::new(multisig_account_id.clone()).with_metadata(metadata));
        executor.visit_register_account(&register_account);
        if executor.verdict().is_err() {
            return executor.verdict().clone();
        }

        configure_roles(executor, &domain_owner, &multisig_account_id, &spec)?;

        Ok(())
    }
}

fn validate_registration<V: Execute + Visit + ?Sized>(
    multisig_account: &AccountId,
    spec: &MultisigSpec,
    executor: &V,
) -> Result<DomainId, ValidationFail> {
    let domain_id = signatory_domain(spec)?;
    if multisig_account.domain() != &domain_id {
        return Err(ValidationFail::NotPermitted(format!(
            "multisig account `{multisig_account}` must belong to domain `{domain_id}`"
        )));
    }
    ensure_quorum_reachable(spec)?;
    ensure_signatories_exist(spec, executor)?;
    ensure_multisig_graph_is_acyclic(spec.signatories.keys().cloned(), executor)?;
    Ok(domain_id)
}

fn signatory_domain(spec: &MultisigSpec) -> Result<DomainId, ValidationFail> {
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

fn ensure_signatories_exist<V: Execute + Visit + ?Sized>(
    spec: &MultisigSpec,
    executor: &V,
) -> Result<(), ValidationFail> {
    for account in spec.signatories.keys() {
        fetch_account_by_id(account, executor)?;
    }
    Ok(())
}

fn account_exists<V: Execute + Visit + ?Sized>(
    account_id: &AccountId,
    executor: &V,
) -> Result<bool, ValidationFail> {
    let predicate = account_id_predicate(account_id)?;
    let accounts = executor
        .host()
        .query(FindAccounts)
        .filter(predicate)
        .execute_all()?;
    Ok(!accounts.is_empty())
}

fn domain_owner<V: Execute + Visit + ?Sized>(
    domain_id: &DomainId,
    executor: &V,
) -> Result<AccountId, ValidationFail> {
    executor
        .host()
        .query(FindDomains)
        .execute_all()?
        .into_iter()
        .find(|domain| domain.id() == domain_id)
        .map(|domain| domain.owned_by().clone())
        .ok_or_else(|| {
            ValidationFail::InstructionFailed(InstructionExecutionError::Find(FindError::Domain(
                domain_id.clone(),
            )))
        })
}

fn configure_roles<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    domain_owner: &AccountId,
    multisig_account: &AccountId,
    spec: &MultisigSpec,
) -> Result<(), ValidationFail> {
    let original_authority = executor.context().authority.clone();
    let signatories: Vec<AccountId> = spec.signatories.keys().cloned().collect();

    let result = (|| {
        executor.context_mut().authority = domain_owner.clone();

        let multisig_role_id = multisig_role_for(multisig_account);
        ensure_role_available(executor, domain_owner, &multisig_role_id, &signatories)?;
        grant_role_if_needed(executor, &multisig_role_id, multisig_account)?;

        for signatory in &signatories {
            let signatory_role_id = multisig_role_for(signatory);
            let delegates = [signatory.clone(), multisig_account.clone()];

            ensure_role_available(executor, domain_owner, &signatory_role_id, &delegates)?;
            grant_role_if_needed(executor, &signatory_role_id, signatory)?;
            grant_role_if_needed(executor, &signatory_role_id, multisig_account)?;
            grant_role_if_needed(executor, &multisig_role_id, signatory)?;
        }

        Ok(())
    })();

    executor.context_mut().authority = original_authority;
    result
}

fn ensure_role_available<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    domain_owner: &AccountId,
    role_id: &RoleId,
    delegates: &[AccountId],
) -> Result<(), ValidationFail> {
    if !role_exists(role_id, executor)? {
        let register_role = Register::role(Role::new(role_id.clone(), domain_owner.clone()));
        executor.visit_register_role(&register_role);
        if executor.verdict().is_err() {
            return executor.verdict().clone();
        }
        return Ok(());
    }

    if has_role(domain_owner, role_id, executor)? {
        return Ok(());
    }

    for delegate in delegates {
        if delegate == domain_owner {
            continue;
        }
        if !has_role(delegate, role_id, executor)? {
            continue;
        }

        let saved = executor.context().authority.clone();
        executor.context_mut().authority = delegate.clone();

        if !has_role(domain_owner, role_id, executor)? {
            let grant = Grant::account_role(role_id.clone(), domain_owner.clone());
            executor.visit_grant_account_role(&grant);
            if executor.verdict().is_err() {
                let verdict = executor.verdict().clone();
                executor.context_mut().authority = saved;
                return verdict;
            }
        }

        executor.context_mut().authority = saved;

        if has_role(domain_owner, role_id, executor)? {
            return Ok(());
        }
    }

    Err(ValidationFail::NotPermitted(format!(
        "domain owner `{domain_owner}` must hold role `{role_id}` to configure multisig"
    )))
}

fn grant_role_if_needed<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    role_id: &RoleId,
    account: &AccountId,
) -> Result<(), ValidationFail> {
    if has_role(account, role_id, executor)? {
        return Ok(());
    }

    let grant = Grant::account_role(role_id.clone(), account.clone());
    executor.visit_grant_account_role(&grant);
    if executor.verdict().is_err() {
        return executor.verdict().clone();
    }

    Ok(())
}

fn role_exists<V: Execute + Visit + ?Sized>(
    role_id: &RoleId,
    executor: &V,
) -> Result<bool, ValidationFail> {
    let roles = executor.host().query(FindRoles).execute_all()?;
    Ok(roles.into_iter().any(|role| role.id() == role_id))
}

fn has_role<V: Execute + Visit + ?Sized>(
    account: &AccountId,
    role_id: &RoleId,
    executor: &V,
) -> Result<bool, ValidationFail> {
    let roles = executor
        .host()
        .query(FindRolesByAccountId::new(account.clone()))
        .execute_all()?;
    Ok(roles.into_iter().any(|candidate| candidate == *role_id))
}

fn ensure_multisig_graph_is_acyclic<V: Execute + Visit + ?Sized>(
    roots: impl IntoIterator<Item = AccountId>,
    executor: &V,
) -> Result<(), ValidationFail> {
    ensure_multisig_graph_is_acyclic_with(roots, |account| {
        if !is_multisig(account, executor)? {
            return Ok(Vec::new());
        }

        let spec = multisig_spec(account, executor)?;
        Ok(spec.signatories.keys().cloned().collect())
    })
}

fn ensure_multisig_graph_is_acyclic_with<F>(
    roots: impl IntoIterator<Item = AccountId>,
    mut fetch_children: F,
) -> Result<(), ValidationFail>
where
    F: FnMut(&AccountId) -> Result<Vec<AccountId>, ValidationFail>,
{
    let mut visiting = Vec::new();
    let mut visited = BTreeSet::new();

    for root in roots {
        dfs(&root, &mut fetch_children, &mut visiting, &mut visited)?;
    }

    Ok(())
}

fn dfs<F>(
    account: &AccountId,
    fetch_children: &mut F,
    visiting: &mut Vec<AccountId>,
    visited: &mut BTreeSet<AccountId>,
) -> Result<(), ValidationFail>
where
    F: FnMut(&AccountId) -> Result<Vec<AccountId>, ValidationFail>,
{
    if visited.contains(account) {
        return Ok(());
    }

    if let Some(pos) = visiting.iter().position(|candidate| candidate == account) {
        let mut cycle = visiting[pos..].to_vec();
        cycle.push(account.clone());
        let path = cycle
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(ValidationFail::NotPermitted(format!(
            "multisig spec forms a cycle: {path}"
        )));
    }

    visiting.push(account.clone());

    for child in fetch_children(account)? {
        dfs(&child, fetch_children, visiting, visited)?;
    }

    visiting.pop();
    visited.insert(account.clone());

    Ok(())
}

#[cfg(test)]
mod tests {
    use core::num::{NonZeroU16, NonZeroU64};
    use std::collections::BTreeMap;

    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;
    use crate::data_model::{domain::DomainId, prelude::AccountId};

    fn account(seed: u8, domain: &DomainId) -> AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(domain.clone(), key_pair.public_key().clone())
    }

    #[test]
    fn signatory_domains_must_match() {
        let domain_a: DomainId = "wonderland".parse().unwrap();
        let domain_b: DomainId = "looking_glass".parse().unwrap();
        let account_a = account(0, &domain_a);
        let account_b = account(1, &domain_b);

        let mut signatories = BTreeMap::new();
        signatories.insert(account_a.clone(), 1);
        signatories.insert(account_b.clone(), 1);

        let spec = MultisigSpec::new(
            signatories,
            NonZeroU16::new(1).unwrap(),
            NonZeroU64::new(1).unwrap(),
        );

        let result = signatory_domain(&spec);
        assert!(matches!(result, Err(ValidationFail::NotPermitted(_))));
    }

    #[test]
    fn quorum_must_be_reachable() {
        let domain: DomainId = "wonderland".parse().unwrap();
        let single = account(0, &domain);
        let mut signatories = BTreeMap::new();
        signatories.insert(single, 1);

        let spec = MultisigSpec::new(
            signatories,
            NonZeroU16::new(2).unwrap(),
            NonZeroU64::new(1).unwrap(),
        );

        let result = ensure_quorum_reachable(&spec);
        assert!(matches!(
            result,
            Err(ValidationFail::NotPermitted(message)) if message.contains("quorum")
        ));
    }

    #[test]
    fn acyclic_graph_passes_validation() {
        let domain: DomainId = "wonderland".parse().unwrap();
        let root = account(0, &domain);
        let child_a = account(1, &domain);
        let child_b = account(2, &domain);
        let leaf = account(3, &domain);

        let mut graph: BTreeMap<AccountId, Vec<AccountId>> = BTreeMap::new();
        graph.insert(root.clone(), vec![child_a.clone(), child_b.clone()]);
        graph.insert(child_a.clone(), vec![leaf.clone()]);
        graph.insert(child_b.clone(), Vec::new());
        graph.insert(leaf.clone(), Vec::new());

        let result = ensure_multisig_graph_is_acyclic_with(vec![root], |account| {
            Ok(graph.get(account).cloned().unwrap_or_default())
        });

        assert!(result.is_ok());
    }

    #[test]
    fn cyclic_graph_is_rejected() {
        let domain: DomainId = "wonderland".parse().unwrap();
        let root = account(0, &domain);
        let child = account(1, &domain);

        let mut graph: BTreeMap<AccountId, Vec<AccountId>> = BTreeMap::new();
        graph.insert(root.clone(), vec![child.clone()]);
        graph.insert(child.clone(), vec![root.clone()]);

        let result = ensure_multisig_graph_is_acyclic_with(vec![root.clone()], |account| {
            Ok(graph.get(account).cloned().unwrap_or_default())
        });

        assert!(matches!(
            result,
            Err(ValidationFail::NotPermitted(message)) if message.contains("multisig spec forms a cycle")
        ));
    }
}
