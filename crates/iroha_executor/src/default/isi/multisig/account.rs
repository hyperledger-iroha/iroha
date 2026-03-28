//! Validation and execution logic of instructions for multisig accounts

use std::collections::BTreeSet;

use iroha_smart_contract::data_model::{
    prelude::{FindAccounts, Grant, Register},
    query::prelude::{FindDomains, FindRoles, FindRolesByAccountId},
    role::{Role, RoleId},
};

use super::*;
use crate::data_model::{
    domain::DomainId, isi::error::InstructionExecutionError, metadata::Metadata, name::Name,
};

impl VisitExecute for MultisigRegister {
    fn visit<V: Execute + Visit + ?Sized>(&self, executor: &mut V) {
        if let Err(err) = validate_registration(
            &self.account,
            self.home_domain.as_ref(),
            &self.spec,
            executor,
        ) {
            deny!(executor, err);
        }
    }

    fn execute<V: Execute + Visit + ?Sized>(self, executor: &mut V) -> Result<(), ValidationFail> {
        let spec = self.spec;
        let multisig_account_id = self.account;
        let home_domain = self.home_domain;
        validate_registration(&multisig_account_id, home_domain.as_ref(), &spec, executor)?;
        let domain_owner = home_domain
            .as_ref()
            .map(|domain_id| domain_owner(domain_id, executor))
            .transpose()?;

        if account_exists(&multisig_account_id, executor)? {
            return Err(ValidationFail::NotPermitted(format!(
                "multisig account `{multisig_account_id}` already exists"
            )));
        }

        let mut metadata = Metadata::default();
        metadata.insert(spec_key(), Json::new(spec.clone()));
        metadata.insert(home_domain_key(), Json::new(home_domain.clone()));

        let register_account = if let Some(home_domain) = home_domain.clone() {
            Register::account(
                Account::new_in_domain(multisig_account_id.clone(), home_domain)
                    .with_metadata(metadata),
            )
        } else {
            Register::account(
                Account::new_domainless(multisig_account_id.clone()).with_metadata(metadata),
            )
        };
        let original_authority = executor.context().authority.clone();
        let register_result = {
            executor.context_mut().authority = domain_owner
                .clone()
                .unwrap_or_else(|| original_authority.clone());
            executor.visit_register_account(&register_account);
            executor.verdict().clone()
        };
        executor.context_mut().authority = original_authority;
        register_result?;

        let role_owner = domain_owner
            .clone()
            .unwrap_or_else(|| multisig_account_id.clone());

        materialize_missing_signatory_accounts(
            executor,
            &role_owner,
            home_domain.as_ref(),
            &multisig_account_id,
            &spec,
        )?;

        configure_roles(
            executor,
            &role_owner,
            home_domain.as_ref(),
            &multisig_account_id,
            &spec,
        )?;

        Ok(())
    }
}

fn validate_registration<V: Execute + Visit + ?Sized>(
    _multisig_account: &AccountId,
    _home_domain: Option<&DomainId>,
    spec: &MultisigSpec,
    executor: &V,
) -> Result<(), ValidationFail> {
    ensure_quorum_reachable(spec)?;
    ensure_multisig_graph_is_acyclic(spec.signatories.keys().cloned(), executor)?;
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

fn multisig_created_via_key() -> Name {
    "iroha:created_via"
        .parse()
        .expect("multisig created_via metadata key must be valid")
}

fn signatories_to_materialize(spec: &MultisigSpec, multisig_account: &AccountId) -> Vec<AccountId> {
    spec.signatories
        .keys()
        .filter(|signatory| signatory.subject_id() != multisig_account.subject_id())
        .cloned()
        .collect()
}

fn materialize_missing_signatory_accounts<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    authority: &AccountId,
    home_domain: Option<&DomainId>,
    multisig_account: &AccountId,
    spec: &MultisigSpec,
) -> Result<(), ValidationFail> {
    let original_authority = executor.context().authority.clone();

    let result = (|| {
        executor.context_mut().authority = authority.clone();

        for signatory in signatories_to_materialize(spec, multisig_account) {
            ensure_signatory_account_exists(executor, &signatory, home_domain)?;
        }

        Ok(())
    })();

    executor.context_mut().authority = original_authority;
    result
}

fn ensure_signatory_account_exists<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    signatory: &AccountId,
    home_domain: Option<&DomainId>,
) -> Result<(), ValidationFail> {
    if account_exists(signatory, executor)? {
        return Ok(());
    }

    let mut metadata = Metadata::default();
    metadata.insert(multisig_created_via_key(), Json::new("multisig"));
    let register_account = if let Some(home_domain) = home_domain.cloned() {
        Register::account(
            Account::new_in_domain(signatory.clone(), home_domain).with_metadata(metadata),
        )
    } else {
        Register::account(Account::new_domainless(signatory.clone()).with_metadata(metadata))
    };
    executor.visit_register_account(&register_account);
    if executor.verdict().is_err() {
        return executor.verdict().clone();
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
    role_owner: &AccountId,
    home_domain: Option<&DomainId>,
    multisig_account: &AccountId,
    spec: &MultisigSpec,
) -> Result<(), ValidationFail> {
    let original_authority = executor.context().authority.clone();
    let signatories: Vec<AccountId> = spec.signatories.keys().cloned().collect();

    let result = (|| {
        executor.context_mut().authority = role_owner.clone();

        let multisig_role_id = multisig_role_for(home_domain, multisig_account);
        ensure_role_available(executor, role_owner, &multisig_role_id, &signatories)?;
        grant_role_if_needed(executor, &multisig_role_id, multisig_account)?;

        for signatory in &signatories {
            let signatory_role_id = multisig_role_for(home_domain, signatory);
            let delegates = [signatory.clone(), multisig_account.clone()];

            ensure_role_available(executor, role_owner, &signatory_role_id, &delegates)?;
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

    fn account(seed: u8, _domain: &DomainId) -> AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    #[test]
    fn signatories_from_multiple_domains_are_allowed() {
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

        let result = ensure_quorum_reachable(&spec);
        assert!(result.is_ok());
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

    #[test]
    fn signatory_materialization_skips_multisig_subject() {
        let domain: DomainId = "wonderland".parse().unwrap();
        let multisig_account = account(0, &domain);
        let signer_one = account(1, &domain);
        let signer_two = account(2, &domain);
        let spec = MultisigSpec::new(
            BTreeMap::from([
                (multisig_account.clone(), 1),
                (signer_one.clone(), 1),
                (signer_two.clone(), 1),
            ]),
            NonZeroU16::new(2).unwrap(),
            NonZeroU64::new(1).unwrap(),
        );

        let materialized = signatories_to_materialize(&spec, &multisig_account);
        assert!(
            materialized
                .iter()
                .all(|account| account.subject_id() != multisig_account.subject_id()),
            "multisig account should never be auto-materialized as its own signatory"
        );
        assert!(
            materialized
                .iter()
                .any(|account| account.subject_id() == signer_one.subject_id()),
            "first external signatory should be materialized"
        );
        assert!(
            materialized
                .iter()
                .any(|account| account.subject_id() == signer_two.subject_id()),
            "second external signatory should be materialized"
        );
    }
}
