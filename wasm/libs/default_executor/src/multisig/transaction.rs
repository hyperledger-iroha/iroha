//! Validation and execution logic of instructions for multisig transactions

use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    vec::Vec,
};

use super::*;

impl VisitExecute for MultisigPropose {
    fn visit(&self, executor: &mut Executor) {
        let host = executor.host();
        let proposer = executor.context().authority.clone();
        let target_account = self.account.clone();
        let multisig_role = multisig_role_for(&target_account);
        let instructions_hash = HashOf::new(&self.instructions);
        let is_top_down_proposal = host
            .query_single(FindAccountMetadata::new(
                proposer.clone(),
                SIGNATORIES.parse().unwrap(),
            ))
            .map_or(false, |proposer_signatories| {
                proposer_signatories
                    .try_into_any::<BTreeMap<AccountId, u8>>()
                    .dbg_unwrap()
                    .contains_key(&target_account)
            });

        if !is_top_down_proposal {
            let Ok(_role_found) = host
                .query(FindRolesByAccountId::new(proposer))
                .filter_with(|role_id| role_id.eq(multisig_role))
                .execute_single()
            else {
                deny!(executor, "not qualified to propose multisig");
            };
        }

        let Err(_proposal_not_found) = host.query_single(FindAccountMetadata::new(
            target_account.clone(),
            approvals_key(&instructions_hash),
        )) else {
            deny!(executor, "multisig proposal duplicates")
        };

        // Pass validation and elevate to the multisig account authority
        executor.context_mut().authority = target_account;
    }

    fn execute(
        self,
        executor: &mut Executor,
        init_authority: &AccountId,
    ) -> Result<(), ValidationFail> {
        let host = executor.host().clone();
        let target_account = self.account;
        let instructions_hash = HashOf::new(&self.instructions);
        let signatories: BTreeMap<AccountId, u8> = host
            .query_single(FindAccountMetadata::new(
                target_account.clone(),
                SIGNATORIES.parse().unwrap(),
            ))
            .dbg_unwrap()
            .try_into_any()
            .dbg_unwrap();

        // Recursively deploy multisig authentication down to the personal leaf signatories
        for signatory in signatories.keys().cloned() {
            let is_multisig_again = host
                .query(FindRoleIds)
                .filter_with(|role_id| role_id.eq(multisig_role_for(&signatory)))
                .execute_single_opt()
                .dbg_unwrap()
                .is_some();

            if is_multisig_again {
                let propose_to_approve_me = {
                    let approve_me =
                        MultisigApprove::new(target_account.clone(), instructions_hash.clone());

                    MultisigPropose::new(signatory, [approve_me.into()].to_vec())
                };
                propose_to_approve_me.visit_execute(executor);
            }
        }

        let now_ms: u64 = executor
            .context()
            .curr_block
            .creation_time()
            .as_millis()
            .try_into()
            .dbg_expect("shouldn't overflow within 584942417 years");

        let approvals = BTreeSet::from([init_authority]);

        host.submit(&SetKeyValue::account(
            target_account.clone(),
            instructions_key(&instructions_hash).clone(),
            Json::new(&self.instructions),
        ))
        .dbg_unwrap();

        host.submit(&SetKeyValue::account(
            target_account.clone(),
            proposed_at_ms_key(&instructions_hash).clone(),
            Json::new(&now_ms),
        ))
        .dbg_unwrap();

        host.submit(&SetKeyValue::account(
            target_account.clone(),
            approvals_key(&instructions_hash).clone(),
            Json::new(&approvals),
        ))
        .dbg_unwrap();

        Ok(())
    }
}

impl VisitExecute for MultisigApprove {
    fn visit(&self, executor: &mut Executor) {
        let host = executor.host();
        let approver = executor.context().authority.clone();
        let target_account = self.account.clone();
        let multisig_role = multisig_role_for(&target_account);
        let instructions_hash = self.instructions_hash;

        let Ok(_role_found) = host
            .query(FindRolesByAccountId::new(approver))
            .filter_with(|role_id| role_id.eq(multisig_role))
            .execute_single()
        else {
            deny!(executor, "not qualified to approve multisig");
        };

        let Ok(_proposal_found) = host.query_single(FindAccountMetadata::new(
            target_account.clone(),
            approvals_key(&instructions_hash),
        )) else {
            deny!(executor, "no proposals to approve")
        };

        // Pass validation and elevate to the multisig account authority
        executor.context_mut().authority = target_account;
    }

    fn execute(
        self,
        executor: &mut Executor,
        init_authority: &AccountId,
    ) -> Result<(), ValidationFail> {
        let host = executor.host().clone();
        let target_account = self.account;
        let instructions_hash = self.instructions_hash;
        let signatories: BTreeMap<AccountId, u8> = host
            .query_single(FindAccountMetadata::new(
                target_account.clone(),
                SIGNATORIES.parse().unwrap(),
            ))
            .dbg_unwrap()
            .try_into_any()
            .dbg_unwrap();
        let quorum: u16 = host
            .query_single(FindAccountMetadata::new(
                target_account.clone(),
                QUORUM.parse().unwrap(),
            ))
            .dbg_unwrap()
            .try_into_any()
            .dbg_unwrap();
        let transaction_ttl_ms: u64 = host
            .query_single(FindAccountMetadata::new(
                target_account.clone(),
                TRANSACTION_TTL_MS.parse().unwrap(),
            ))
            .dbg_unwrap()
            .try_into_any()
            .dbg_unwrap();
        let instructions: Vec<InstructionBox> = host
            .query_single(FindAccountMetadata::new(
                target_account.clone(),
                instructions_key(&instructions_hash),
            ))
            .dbg_unwrap()
            .try_into_any()
            .dbg_unwrap();
        let proposed_at_ms: u64 = host
            .query_single(FindAccountMetadata::new(
                target_account.clone(),
                proposed_at_ms_key(&instructions_hash),
            ))
            .dbg_unwrap()
            .try_into_any()
            .dbg_unwrap();
        let mut approvals: BTreeSet<AccountId> = host
            .query_single(FindAccountMetadata::new(
                target_account.clone(),
                approvals_key(&instructions_hash),
            ))
            .dbg_unwrap()
            .try_into_any()
            .dbg_unwrap();

        approvals.insert(init_authority.clone());

        host.submit(&SetKeyValue::account(
            target_account.clone(),
            approvals_key(&instructions_hash),
            Json::new(&approvals),
        ))
        .dbg_unwrap();

        let now_ms: u64 = executor
            .context()
            .curr_block
            .creation_time()
            .as_millis()
            .try_into()
            .dbg_expect("shouldn't overflow within 584942417 years");

        let is_authenticated = quorum
            <= signatories
                .into_iter()
                .filter(|(id, _)| approvals.contains(&id))
                .map(|(_, weight)| weight as u16)
                .sum();

        let is_expired = proposed_at_ms.saturating_add(transaction_ttl_ms) < now_ms;

        if is_authenticated || is_expired {
            // Cleanup the transaction entry
            host.submit(&RemoveKeyValue::account(
                target_account.clone(),
                approvals_key(&instructions_hash),
            ))
            .dbg_unwrap();

            host.submit(&RemoveKeyValue::account(
                target_account.clone(),
                proposed_at_ms_key(&instructions_hash),
            ))
            .dbg_unwrap();

            host.submit(&RemoveKeyValue::account(
                target_account.clone(),
                instructions_key(&instructions_hash),
            ))
            .dbg_unwrap();

            if !is_expired {
                // Execute instructions proposal which collected enough approvals
                for isi in instructions {
                    match isi {
                        InstructionBox::Custom(instruction) => visit_custom(executor, &instruction),
                        builtin => host.submit(&builtin).dbg_unwrap(),
                    }
                }
            } else {
                // TODO Notify that the proposal has expired, while returning Ok for the entry deletion to take effect
            }
        }

        Ok(())
    }
}