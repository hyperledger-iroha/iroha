//! Validation and execution logic of instructions for multisig transactions

use std::collections::BTreeSet;

use iroha_smart_contract::data_model::{isi::CustomInstruction, query::error::QueryExecutionFail};

use super::*;
use crate::{
    data_model::{Level, query::error::FindError},
    smart_contract::DebugExpectExt as _,
};

fn proposer_is_authorized(
    multisig_account: &AccountId,
    proposer: &AccountId,
    spec: &MultisigSpec,
    is_downward_proposal: bool,
    has_multisig_role: bool,
) -> bool {
    is_downward_proposal
        || has_multisig_role
        || proposer == multisig_account
        || spec.signatories.contains_key(proposer)
}

fn approver_is_authorized(
    multisig_account: &AccountId,
    approver: &AccountId,
    spec: &MultisigSpec,
    has_multisig_role: bool,
) -> bool {
    has_multisig_role || approver == multisig_account || spec.signatories.contains_key(approver)
}

fn canceler_is_authorized(multisig_account: &AccountId, canceler: &AccountId) -> bool {
    canceler.subject_id() == multisig_account.subject_id()
}

impl VisitExecute for MultisigPropose {
    fn visit<V: Execute + Visit + ?Sized>(&self, executor: &mut V) {
        let host = executor.host();
        let proposer = executor.context().authority.clone();
        let multisig_account = self.account.clone();
        let instructions_hash = HashOf::new(&self.instructions);
        let multisig_spec = match multisig_spec(&multisig_account, executor) {
            Ok(spec) => spec,
            Err(err) => deny!(executor, err),
        };
        let home_domain = match multisig_home_domain(&multisig_account, executor) {
            Ok(home_domain) => home_domain,
            Err(err) => deny!(executor, err),
        };
        let proposer_role = multisig_role_for(&home_domain, &proposer);
        let multisig_role = multisig_role_for(&home_domain, &multisig_account);
        let is_downward_proposal = host
            .query(FindRolesByAccountId::new(multisig_account.clone()))
            .execute_all()
            .map(|roles| roles.into_iter().any(|role| role == proposer_role))
            .unwrap_or(false);
        let has_multisig_role = host
            .query(FindRolesByAccountId::new(proposer.clone()))
            .execute_all()
            .map(|roles| roles.into_iter().any(|role| role == multisig_role))
            .unwrap_or(false);
        let has_not_longer_ttl = self
            .transaction_ttl_ms
            .is_none_or(|override_ttl_ms| override_ttl_ms <= multisig_spec.transaction_ttl_ms);

        if !has_not_longer_ttl {
            deny!(executor, "ttl violates the restriction");
        }

        if !proposer_is_authorized(
            &multisig_account,
            &proposer,
            &multisig_spec,
            is_downward_proposal,
            has_multisig_role,
        ) {
            deny!(executor, "not qualified to propose multisig");
        }

        match proposal_value(&multisig_account, instructions_hash, executor) {
            Ok(existing) if now_ms(executor) < existing.expires_at_ms => {
                deny!(executor, "multisig proposal duplicates")
            }
            Ok(_)
            | Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::MetadataKey(
                _,
            )))) => {}
            Err(err) => deny!(executor, err),
        }
    }

    fn execute<V: Execute + Visit + ?Sized>(self, executor: &mut V) -> Result<(), ValidationFail> {
        let proposer = executor.context().authority.clone();
        let multisig_account = self.account;
        let instructions_hash = HashOf::new(&self.instructions);
        let spec = multisig_spec(&multisig_account, executor)?;

        let now_ms = now_ms(executor);
        if proposal_value(&multisig_account, instructions_hash, executor).is_ok() {
            prune_expired(multisig_account.clone(), instructions_hash, executor)?;
        }
        let expires_at_ms = {
            let ttl_ms = self.transaction_ttl_ms.unwrap_or(spec.transaction_ttl_ms);
            now_ms.saturating_add(ttl_ms.into())
        };
        let proposal_value = MultisigProposalValue::new(
            self.instructions,
            now_ms,
            expires_at_ms,
            BTreeSet::from([proposer]),
            None,
        );

        let approve_me = MultisigApprove::new(multisig_account.clone(), instructions_hash);
        // Recursively deploy multisig authentication down to the personal leaf signatories
        for signatory in spec.signatories.keys().cloned() {
            if is_multisig(&signatory, executor)? {
                deploy_relayer(
                    signatory,
                    approve_me.clone(),
                    now_ms,
                    expires_at_ms,
                    executor,
                )?;
            }
        }

        // Authorize as the multisig account
        executor.context_mut().authority = multisig_account.clone();

        visit_seq!(executor.visit_set_account_key_value(&SetKeyValue::account(
            multisig_account,
            proposal_key(&instructions_hash),
            proposal_value,
        )));

        Ok(())
    }
}

fn deploy_relayer<V: Execute + Visit + ?Sized>(
    relayer: AccountId,
    relay: MultisigApprove,
    now_ms: u64,
    parent_expires_at_ms: u64,
    executor: &mut V,
) -> Result<(), ValidationFail> {
    let spec = multisig_spec(&relayer, executor)?;
    let relay_expires_at_ms =
        capped_relay_expiry(now_ms, parent_expires_at_ms, spec.transaction_ttl_ms.get());

    let relay_hash = HashOf::new(&vec![relay.clone().into()]);
    let sub_relay = MultisigApprove::new(relayer.clone(), relay_hash);

    for signatory in spec.signatories.keys().cloned() {
        if is_multisig(&signatory, executor)? {
            deploy_relayer(
                signatory,
                sub_relay.clone(),
                now_ms,
                relay_expires_at_ms,
                executor,
            )?;
        }
    }

    // Authorize as the relayer account
    executor.context_mut().authority = relayer.clone();

    let relay_value = MultisigProposalValue::new(
        vec![relay.into()],
        now_ms,
        relay_expires_at_ms,
        BTreeSet::new(),
        Some(false),
    );
    visit_seq!(executor.visit_set_account_key_value(&SetKeyValue::account(
        relayer,
        proposal_key(&relay_hash),
        relay_value,
    )));

    Ok(())
}

fn capped_relay_expiry(now_ms: u64, parent_expires_at_ms: u64, relayer_ttl_ms: u64) -> u64 {
    let local_expiry = now_ms.saturating_add(relayer_ttl_ms);
    local_expiry.min(parent_expires_at_ms)
}

fn proposal_value<V: Execute + Visit + ?Sized>(
    multisig_account: &AccountId,
    instructions_hash: HashOf<Vec<InstructionBox>>,
    executor: &V,
) -> Result<MultisigProposalValue, ValidationFail> {
    let key = proposal_key(&instructions_hash);
    super::load_account_metadata(multisig_account, &key, executor)?
        .try_into_any_norito()
        .map_err(metadata_conversion_error)
}

fn now_ms<V: Execute + Visit + ?Sized>(executor: &V) -> u64 {
    executor
        .context()
        .curr_block
        .creation_time()
        .as_millis()
        .try_into()
        .dbg_expect("shouldn't overflow within 584942417 years")
}

fn ensure_not_derived_multisig_account(
    multisig_account: &AccountId,
    spec: &MultisigSpec,
) -> Result<(), ValidationFail> {
    if spec.signatories.is_empty() {
        return Err(ValidationFail::NotPermitted(
            "multisig spec must include at least one signatory".to_owned(),
        ));
    }
    // TODO: Reject deterministically derived multisig account ids once the derivation
    // inputs are finalized for the domainless AccountId model.
    let _ = multisig_account;
    Ok(())
}

impl VisitExecute for MultisigApprove {
    fn visit<V: Execute + Visit + ?Sized>(&self, executor: &mut V) {
        let approver = executor.context().authority.clone();
        let multisig_account = self.account.clone();
        let host = executor.host();
        let instructions_hash = self.instructions_hash;

        let spec = match multisig_spec(&multisig_account, executor) {
            Ok(spec) => spec,
            Err(err) => deny!(executor, err),
        };
        let home_domain = match multisig_home_domain(&multisig_account, executor) {
            Ok(home_domain) => home_domain,
            Err(err) => deny!(executor, err),
        };
        let has_multisig_role = host
            .query(FindRolesByAccountId::new(approver.clone()))
            .execute_all()
            .map(|roles| {
                roles
                    .into_iter()
                    .any(|role| role == multisig_role_for(&home_domain, &multisig_account))
            })
            .unwrap_or(false);

        if !approver_is_authorized(&multisig_account, &approver, &spec, has_multisig_role) {
            deny!(executor, "not qualified to approve multisig");
        }

        if let Err(err) = ensure_not_derived_multisig_account(&multisig_account, &spec) {
            deny!(executor, err);
        }

        if let Err(err) = proposal_value(&multisig_account, instructions_hash, executor) {
            deny!(executor, err)
        }
    }

    fn execute<V: Execute + Visit + ?Sized>(self, executor: &mut V) -> Result<(), ValidationFail> {
        let approver = executor.context().authority.clone();
        let multisig_account = self.account;
        let instructions_hash = self.instructions_hash;

        // Check if the proposal is expired
        // Authorize as the multisig account
        prune_expired(multisig_account.clone(), instructions_hash, executor)?;

        let Ok(mut proposal_value) = proposal_value(&multisig_account, instructions_hash, executor)
        else {
            // The proposal is pruned
            // Notify that the proposal has expired, while returning Ok for the entry deletion to take effect
            let log = Log::new(
                Level::INFO,
                format!(
                    "multisig proposal expired:\naccount: {multisig_account}\ninstructions hash: {instructions_hash}"
                ),
            );
            visit_seq!(executor.visit_log(&log));
            return Ok(());
        };
        if let Some(true) = proposal_value.is_relayed {
            // The relaying approval already has executed
            return Ok(());
        }

        let spec = multisig_spec(&multisig_account, executor)?;
        ensure_not_derived_multisig_account(&multisig_account, &spec)?;

        proposal_value.approvals.insert(approver);
        visit_seq!(executor.visit_set_account_key_value(&SetKeyValue::account(
            multisig_account.clone(),
            proposal_key(&instructions_hash),
            proposal_value.clone(),
        )));

        let is_authenticated = u16::from(spec.quorum)
            <= spec
                .signatories
                .into_iter()
                .filter(|(id, _)| proposal_value.approvals.contains(id))
                .map(|(_, weight)| u16::from(weight))
                .sum();

        if is_authenticated {
            match proposal_value.is_relayed {
                None => {
                    // Cleanup the transaction entry
                    prune_down(multisig_account, instructions_hash, executor)?;
                }
                Some(false) => {
                    // Mark the relaying approval as executed
                    proposal_value.is_relayed = Some(true);
                    visit_seq!(executor.visit_set_account_key_value(&SetKeyValue::account(
                        multisig_account,
                        proposal_key(&instructions_hash),
                        proposal_value.clone(),
                    )));
                }
                _ => unreachable!(),
            }

            for instruction in proposal_value.instructions {
                visit_seq!(executor.visit_instruction(&instruction));
            }
        }

        Ok(())
    }
}

impl VisitExecute for MultisigCancel {
    fn visit<V: Execute + Visit + ?Sized>(&self, executor: &mut V) {
        let canceler = executor.context().authority.clone();
        let multisig_account = self.account.clone();
        let instructions_hash = self.instructions_hash;

        let spec = match multisig_spec(&multisig_account, executor) {
            Ok(spec) => spec,
            Err(err) => deny!(executor, err),
        };

        if !canceler_is_authorized(&multisig_account, &canceler) {
            deny!(
                executor,
                "multisig cancel must execute as the multisig account"
            );
        }

        if let Err(err) = ensure_not_derived_multisig_account(&multisig_account, &spec) {
            deny!(executor, err);
        }

        if let Err(err) = proposal_value(&multisig_account, instructions_hash, executor) {
            deny!(executor, err);
        }
    }

    fn execute<V: Execute + Visit + ?Sized>(self, executor: &mut V) -> Result<(), ValidationFail> {
        let canceler = executor.context().authority.clone();
        let multisig_account = self.account;
        let instructions_hash = self.instructions_hash;

        let spec = multisig_spec(&multisig_account, executor)?;
        if !canceler_is_authorized(&multisig_account, &canceler) {
            return Err(ValidationFail::NotPermitted(
                "multisig cancel must execute as the multisig account".to_owned(),
            ));
        }
        ensure_not_derived_multisig_account(&multisig_account, &spec)?;

        prune_expired(multisig_account.clone(), instructions_hash, executor)?;

        let proposal_value = proposal_value(&multisig_account, instructions_hash, executor)?;
        if let Some(true) = proposal_value.is_relayed {
            return Err(ValidationFail::NotPermitted(
                "cannot cancel an executed relayed approval".to_owned(),
            ));
        }

        prune_down(multisig_account, instructions_hash, executor)
    }
}

/// Remove an expired proposal and relevant entries, switching the executor authority to this multisig account
fn prune_expired<V: Execute + Visit + ?Sized>(
    multisig_account: AccountId,
    instructions_hash: HashOf<Vec<InstructionBox>>,
    executor: &mut V,
) -> Result<(), ValidationFail> {
    let proposal_value = proposal_value(&multisig_account, instructions_hash, executor)?;

    if now_ms(executor) < proposal_value.expires_at_ms {
        // Authorize as the multisig account
        executor.context_mut().authority = multisig_account.clone();
        return Ok(());
    }

    // Go upstream to the root through approvals
    for instruction in proposal_value.instructions {
        if let Some(custom) = instruction.as_any().downcast_ref::<CustomInstruction>()
            && let Ok(MultisigInstructionBox::Approve(approve)) = custom.payload().try_into()
        {
            return prune_expired(approve.account, approve.instructions_hash, executor);
        }
    }

    // Go downstream, cleaning up relayers
    prune_down(multisig_account, instructions_hash, executor)
}

/// Remove an proposal and relevant entries, switching the executor authority to this multisig account
fn prune_down<V: Execute + Visit + ?Sized>(
    multisig_account: AccountId,
    instructions_hash: HashOf<Vec<InstructionBox>>,
    executor: &mut V,
) -> Result<(), ValidationFail> {
    let spec = multisig_spec(&multisig_account, executor)?;

    // Authorize as the multisig account
    executor.context_mut().authority = multisig_account.clone();

    visit_seq!(
        executor.visit_remove_account_key_value(&RemoveKeyValue::account(
            multisig_account.clone(),
            proposal_key(&instructions_hash),
        ))
    );

    for signatory in spec.signatories.keys().cloned() {
        let relay_hash = {
            let relay = MultisigApprove::new(multisig_account.clone(), instructions_hash);
            HashOf::new(&vec![relay.into()])
        };
        if is_multisig(&signatory, executor)? {
            prune_down(signatory, relay_hash, executor)?
        }
    }

    // Restore the authority
    executor.context_mut().authority = multisig_account;

    Ok(())
}

#[cfg(test)]
mod tests {
    use core::num::{NonZeroU16, NonZeroU64};
    use std::collections::BTreeMap;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_smart_contract::data_model::{account::AccountId, domain::DomainId};

    use super::*;

    fn account(seed: u8, _domain: &DomainId) -> AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn sample_spec(_domain: &DomainId, signer: &AccountId) -> MultisigSpec {
        MultisigSpec::new(
            BTreeMap::from([(signer.clone(), 1)]),
            NonZeroU16::new(1).expect("nonzero quorum"),
            NonZeroU64::new(1).expect("nonzero ttl"),
        )
    }

    #[test]
    fn proposer_authorized_by_signatory_or_self() {
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let signer = account(1, &domain);
        let multisig = account(2, &domain);
        let other = account(3, &domain);
        let spec = sample_spec(&domain, &signer);

        assert!(
            proposer_is_authorized(&multisig, &signer, &spec, false, false),
            "signatory should be authorized"
        );
        assert!(
            proposer_is_authorized(&multisig, &multisig, &spec, false, false),
            "multisig account should be authorized"
        );
        assert!(
            !proposer_is_authorized(&multisig, &other, &spec, false, false),
            "non-signatory should not be authorized"
        );
        assert!(
            proposer_is_authorized(&multisig, &other, &spec, true, false),
            "downward proposal should authorize"
        );
        assert!(
            proposer_is_authorized(&multisig, &other, &spec, false, true),
            "role-based authorization should allow proposer"
        );
    }

    #[test]
    fn approver_authorized_by_signatory_or_self() {
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let signer = account(4, &domain);
        let multisig = account(5, &domain);
        let other = account(6, &domain);
        let spec = sample_spec(&domain, &signer);

        assert!(
            approver_is_authorized(&multisig, &signer, &spec, false),
            "signatory should be authorized"
        );
        assert!(
            approver_is_authorized(&multisig, &multisig, &spec, false),
            "multisig account should be authorized"
        );
        assert!(
            !approver_is_authorized(&multisig, &other, &spec, false),
            "non-signatory should not be authorized"
        );
        assert!(
            approver_is_authorized(&multisig, &other, &spec, true),
            "role-based authorization should allow approver"
        );
    }

    #[test]
    fn canceler_must_be_the_multisig_subject() {
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let multisig = account(10, &domain);
        let same_subject = multisig.clone();
        let other = account(11, &domain);

        assert!(
            canceler_is_authorized(&multisig, &same_subject),
            "the multisig account itself should be allowed to execute cancel"
        );
        assert!(
            !canceler_is_authorized(&multisig, &other),
            "signers must not be able to execute cancel directly outside multisig"
        );
    }

    #[test]
    fn derived_multisig_account_is_rejected() {
        let domain: DomainId = "derived".parse().expect("valid domain");
        let signer = account(7, &domain);
        let spec = sample_spec(&domain, &signer);
        let seed = HashOf::<(DomainId, MultisigSpec)>::new(&(domain.clone(), spec.clone()));
        let derived = KeyPair::from_seed(seed.as_ref().to_vec(), Algorithm::Ed25519);
        let multisig_account = AccountId::new(derived.public_key().clone());

        let err = ensure_not_derived_multisig_account(&multisig_account, &spec)
            .expect_err("derived multisig should be rejected");
        match &err {
            ValidationFail::NotPermitted(message) => {
                assert!(message.contains("derived"), "unexpected error: {message}")
            }
            _ => panic!("unexpected error: {err:?}"),
        }
    }

    #[test]
    fn non_derived_multisig_account_is_allowed() {
        let domain: DomainId = "non-derived".parse().expect("valid domain");
        let signer = account(8, &domain);
        let spec = sample_spec(&domain, &signer);
        let seed = HashOf::<(DomainId, MultisigSpec)>::new(&(domain.clone(), spec.clone()));
        let derived = KeyPair::from_seed(seed.as_ref().to_vec(), Algorithm::Ed25519);
        let derived_id = AccountId::new(derived.public_key().clone());
        let multisig_account = account(9, &domain);

        assert_ne!(
            multisig_account, derived_id,
            "sanity check: derived id should differ from chosen multisig id"
        );
        ensure_not_derived_multisig_account(&multisig_account, &spec)
            .expect("non-derived multisig should be allowed");
    }
}
