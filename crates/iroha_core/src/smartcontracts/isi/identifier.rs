//! Hidden-function-backed identifier policy instruction handlers.

use iroha_data_model::{
    identifier::{IdentifierClaimRecord, IdentifierPolicy},
    prelude::*,
};
use iroha_telemetry::metrics;

use super::prelude::*;

/// Execution handlers for identifier-policy ISIs.
pub mod isi {
    use super::*;
    use crate::state::StateTransaction;

    impl Execute for iroha_data_model::isi::identifier::RegisterIdentifierPolicy {
        #[metrics(+"register_identifier_policy")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let policy = self.policy;
            if authority != &policy.owner {
                return Err(Error::InvariantViolation(
                    "Only the policy owner can register an identifier policy"
                        .to_owned()
                        .into(),
                ));
            }
            if state_transaction
                .world
                .identifier_policies
                .get(&policy.id)
                .is_some()
            {
                return Err(Error::InvariantViolation(
                    format!("Identifier policy {} is already registered", policy.id).into(),
                ));
            }
            state_transaction
                .world
                .identifier_policies
                .insert(policy.id.clone(), policy);
            Ok(())
        }
    }

    impl Execute for iroha_data_model::isi::identifier::ActivateIdentifierPolicy {
        #[metrics(+"activate_identifier_policy")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let policy = state_transaction
                .world
                .identifier_policies
                .get_mut(&self.policy_id)
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        format!("Identifier policy {} is not registered", self.policy_id).into(),
                    )
                })?;
            if authority != &policy.owner {
                return Err(Error::InvariantViolation(
                    "Only the policy owner can activate an identifier policy"
                        .to_owned()
                        .into(),
                ));
            }
            if policy.active {
                return Err(Error::InvariantViolation(
                    format!("Identifier policy {} is already active", policy.id).into(),
                ));
            }
            policy.active = true;
            Ok(())
        }
    }

    impl Execute for iroha_data_model::isi::identifier::ClaimIdentifier {
        #[metrics(+"claim_identifier")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let receipt = self.receipt;
            let policy = state_transaction
                .world
                .identifier_policies
                .get(&receipt.policy_id)
                .cloned()
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        format!("Identifier policy {} is not registered", receipt.policy_id).into(),
                    )
                })?;
            ensure_policy_authorized(authority, &policy, Some(&self.account))?;
            if !policy.active {
                return Err(Error::InvariantViolation(
                    format!("Identifier policy {} is not active", policy.id).into(),
                ));
            }
            if let Some(expires_at_ms) = receipt.expires_at_ms
                && expires_at_ms <= receipt.resolved_at_ms
            {
                return Err(Error::InvariantViolation(
                    "identifier receipt expiry must be greater than resolved_at_ms"
                        .to_owned()
                        .into(),
                ));
            }
            receipt.verify(&policy.resolver_public_key).map_err(|err| {
                Error::InvariantViolation(
                    format!(
                        "Identifier receipt signature is invalid for policy {}: {err}",
                        policy.id
                    )
                    .into(),
                )
            })?;

            let uaid = *state_transaction
                .world
                .account(&self.account)
                .map_err(Error::from)?
                .uaid()
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        format!("Account {} does not have a UAID", self.account).into(),
                    )
                })?;
            if receipt.account_id != self.account {
                return Err(Error::InvariantViolation(
                    format!(
                        "Identifier receipt account {} does not match claim account {}",
                        receipt.account_id, self.account
                    )
                    .into(),
                ));
            }
            if receipt.uaid != uaid {
                return Err(Error::InvariantViolation(
                    format!(
                        "Identifier receipt UAID {} does not match account {} UAID {uaid}",
                        receipt.uaid, self.account
                    )
                    .into(),
                ));
            }
            let now_ms = state_transaction.block_unix_timestamp_ms();
            if receipt.resolved_at_ms > now_ms {
                return Err(Error::InvariantViolation(
                    format!(
                        "Identifier receipt for policy {} was issued in the future ({}) relative to block time ({now_ms})",
                        policy.id, receipt.resolved_at_ms
                    )
                    .into(),
                ));
            }
            if receipt
                .expires_at_ms
                .is_some_and(|expires_at_ms| expires_at_ms <= now_ms)
            {
                return Err(Error::InvariantViolation(
                    format!(
                        "Identifier receipt for policy {} expired at or before block time {now_ms}",
                        policy.id
                    )
                    .into(),
                ));
            }

            if let Some(existing_uaid) =
                state_transaction.world.opaque_uaids.get(&receipt.opaque_id)
            {
                if existing_uaid != &uaid {
                    return Err(Error::InvariantViolation(
                        format!(
                            "Opaque identifier {} is already bound to UAID {existing_uaid}",
                            receipt.opaque_id
                        )
                        .into(),
                    ));
                }
            }

            if let Some(existing_claim) = state_transaction
                .world
                .identifier_claims
                .get(&receipt.opaque_id)
            {
                if existing_claim.policy_id != receipt.policy_id
                    || existing_claim.uaid != uaid
                    || existing_claim.account_id != self.account
                {
                    return Err(Error::InvariantViolation(
                        format!(
                            "Opaque identifier {} is already claimed under a different binding",
                            receipt.opaque_id
                        )
                        .into(),
                    ));
                }
            }

            let details = state_transaction
                .world
                .account_mut(&self.account)
                .map_err(Error::from)?;
            let mut opaque_ids = details.opaque_ids().to_vec();
            if !opaque_ids.contains(&receipt.opaque_id) {
                opaque_ids.push(receipt.opaque_id);
                details.set_opaque_ids(opaque_ids);
            }

            state_transaction
                .world
                .opaque_uaids
                .insert(receipt.opaque_id, uaid);
            state_transaction.world.identifier_claims.insert(
                receipt.opaque_id,
                IdentifierClaimRecord {
                    policy_id: receipt.policy_id,
                    opaque_id: receipt.opaque_id,
                    uaid,
                    account_id: self.account,
                    verified_at_ms: receipt.resolved_at_ms,
                    expires_at_ms: receipt.expires_at_ms,
                },
            );
            Ok(())
        }
    }

    impl Execute for iroha_data_model::isi::identifier::RevokeIdentifier {
        #[metrics(+"revoke_identifier")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let claim = state_transaction
                .world
                .identifier_claims
                .get(&self.opaque_id)
                .cloned()
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        format!("Identifier claim {} is not registered", self.opaque_id).into(),
                    )
                })?;
            if claim.policy_id != self.policy_id {
                return Err(Error::InvariantViolation(
                    format!(
                        "Opaque identifier {} is not registered under policy {}",
                        self.opaque_id, self.policy_id
                    )
                    .into(),
                ));
            }

            let policy = state_transaction
                .world
                .identifier_policies
                .get(&self.policy_id)
                .cloned();
            if let Some(policy) = &policy {
                ensure_policy_authorized(authority, policy, Some(&claim.account_id))?;
            } else if authority != &claim.account_id {
                return Err(Error::InvariantViolation(
                    "Only the claimed account can revoke an identifier when the policy is missing"
                        .to_owned()
                        .into(),
                ));
            }

            let details = state_transaction
                .world
                .account_mut(&claim.account_id)
                .map_err(Error::from)?;
            let retained: Vec<_> = details
                .opaque_ids()
                .iter()
                .copied()
                .filter(|opaque| opaque != &self.opaque_id)
                .collect();
            details.set_opaque_ids(retained);

            state_transaction.world.opaque_uaids.remove(self.opaque_id);
            state_transaction
                .world
                .identifier_claims
                .remove(self.opaque_id);
            Ok(())
        }
    }

    fn ensure_policy_authorized(
        authority: &AccountId,
        policy: &IdentifierPolicy,
        account: Option<&AccountId>,
    ) -> Result<(), Error> {
        if authority == &policy.owner || account.is_some_and(|account| authority == account) {
            return Ok(());
        }
        Err(Error::InvariantViolation(
            "Authority is not allowed to mutate this identifier binding"
                .to_owned()
                .into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use iroha_crypto::{Hash, KeyPair, Signature, SignatureOf, policy_commitment};
    use iroha_data_model::{
        IntoKeyValue,
        account::{Account, AccountId, OpaqueAccountId},
        block::BlockHeader,
        domain::DomainId,
        identifier::{
            IdentifierNormalization, IdentifierPolicy, IdentifierPolicyId,
            IdentifierResolutionReceipt, IdentifierResolutionReceiptPayload,
        },
        isi::identifier::{
            ActivateIdentifierPolicy, ClaimIdentifier, RegisterIdentifierPolicy, RevokeIdentifier,
        },
        metadata::Metadata,
        nexus::UniversalAccountId,
        prelude::Domain,
    };
    use mv::storage::StorageReadOnly;
    use nonzero_ext::nonzero;

    use crate::{
        kura::Kura, prelude::World, query::store::LiveQueryStore, smartcontracts::Execute,
        state::State,
    };

    fn test_state() -> State {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        State::new_for_testing(World::default(), kura, query)
    }

    fn seed_domain(state: &mut State, domain_id: &DomainId, owner: &AccountId) {
        let domain = Domain {
            id: domain_id.clone(),
            logo: None,
            metadata: Metadata::default(),
            owned_by: owner.clone(),
        };
        state.world.domains.insert(domain_id.clone(), domain);
    }

    fn seed_account_with_uaid(
        state: &mut State,
        account_id: &AccountId,
        domain_id: &DomainId,
        uaid: UniversalAccountId,
    ) {
        let account = Account {
            id: account_id.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: Some(uaid),
            opaque_ids: Vec::new(),
            linked_domains: BTreeSet::from([domain_id.clone()]),
        };
        let (account_id, account_value) = account.into_key_value();
        let subject = account_id.subject_id();
        state
            .world
            .accounts
            .insert(account_id.clone(), account_value);
        state.world.uaid_accounts.insert(uaid, account_id.clone());
        state
            .world
            .account_subject_domains
            .insert(subject.clone(), BTreeSet::from([domain_id.clone()]));
        state
            .world
            .domain_account_subjects
            .insert(domain_id.clone(), BTreeSet::from([subject]));
    }

    fn claim_receipt(
        policy_id: &IdentifierPolicyId,
        resolver: &KeyPair,
        opaque_id: OpaqueAccountId,
        uaid: UniversalAccountId,
        account_id: &AccountId,
        resolved_at_ms: u64,
        expires_at_ms: Option<u64>,
    ) -> IdentifierResolutionReceipt {
        let payload = IdentifierResolutionReceiptPayload {
            policy_id: policy_id.clone(),
            opaque_id,
            uaid,
            account_id: account_id.clone(),
            resolved_at_ms,
            expires_at_ms,
        };
        let signature: Signature = SignatureOf::new(resolver.private_key(), &payload).into();
        IdentifierResolutionReceipt {
            policy_id: payload.policy_id,
            opaque_id: payload.opaque_id,
            uaid: payload.uaid,
            account_id: payload.account_id,
            resolved_at_ms: payload.resolved_at_ms,
            expires_at_ms: payload.expires_at_ms,
            signature,
        }
    }

    #[test]
    fn identifier_claim_and_revoke_update_indexes() {
        let mut state = test_state();
        let domain_id: DomainId = "directory".parse().expect("domain id");
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-owner"));
        seed_domain(&mut state, &domain_id, &owner);
        seed_account_with_uaid(&mut state, &owner, &domain_id, uaid);

        let resolver = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let policy = IdentifierPolicy::new(
            policy_id.clone(),
            owner.clone(),
            IdentifierNormalization::PhoneE164,
            policy_commitment(b"resolver-secret", policy_id.to_string().into_bytes())
                .expect("commitment"),
            resolver.public_key().clone(),
        );
        let opaque_id = OpaqueAccountId::from(Hash::new(b"+15551234567"));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        RegisterIdentifierPolicy {
            policy: policy.clone(),
        }
        .execute(&owner, &mut tx)
        .expect("register policy");
        ActivateIdentifierPolicy {
            policy_id: policy_id.clone(),
        }
        .execute(&owner, &mut tx)
        .expect("activate policy");
        let receipt = claim_receipt(
            &policy_id,
            &resolver,
            opaque_id,
            uaid,
            &owner,
            0,
            Some(1_800_000_000),
        );
        ClaimIdentifier {
            account: owner.clone(),
            receipt,
        }
        .execute(&owner, &mut tx)
        .expect("claim identifier");
        tx.apply();
        block.commit().expect("commit block");

        let claims = state.world.identifier_claims.view();
        let claim = claims.get(&opaque_id).expect("claim should be indexed");
        assert_eq!(claim.policy_id, policy_id);
        assert_eq!(claim.uaid, uaid);
        assert_eq!(claim.account_id, owner);
        assert_eq!(
            state.world.opaque_uaids.view().get(&opaque_id),
            Some(&uaid),
            "opaque id should resolve to the seeded UAID"
        );
        assert!(
            state
                .world
                .accounts
                .view()
                .get(&owner)
                .expect("account exists")
                .opaque_ids()
                .contains(&opaque_id),
            "account should advertise claimed opaque id"
        );

        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        RevokeIdentifier {
            policy_id: claim.policy_id.clone(),
            opaque_id,
        }
        .execute(&owner, &mut tx)
        .expect("revoke identifier");
        tx.apply();
        block.commit().expect("commit block");

        assert!(
            state
                .world
                .identifier_claims
                .view()
                .get(&opaque_id)
                .is_none(),
            "claim index should be cleared after revoke"
        );
        assert!(
            state.world.opaque_uaids.view().get(&opaque_id).is_none(),
            "opaque index should be cleared after revoke"
        );
        assert!(
            !state
                .world
                .accounts
                .view()
                .get(&owner)
                .expect("account exists")
                .opaque_ids()
                .contains(&opaque_id),
            "account should no longer advertise revoked opaque id"
        );
    }

    #[test]
    fn claim_identifier_rejects_accounts_without_uaid() {
        let mut state = test_state();
        let domain_id: DomainId = "directory".parse().expect("domain id");
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        seed_domain(&mut state, &domain_id, &owner);
        let account = Account {
            id: owner.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
            linked_domains: BTreeSet::from([domain_id.clone()]),
        };
        let (account_id, account_value) = account.into_key_value();
        let subject = account_id.subject_id();
        state
            .world
            .accounts
            .insert(account_id.clone(), account_value);
        state
            .world
            .account_subject_domains
            .insert(subject.clone(), BTreeSet::from([domain_id.clone()]));
        state
            .world
            .domain_account_subjects
            .insert(domain_id, BTreeSet::from([subject]));

        let resolver = KeyPair::random();
        let policy_id: IdentifierPolicyId = "email#retail".parse().expect("policy id");
        let policy = IdentifierPolicy::new(
            policy_id.clone(),
            owner.clone(),
            IdentifierNormalization::EmailAddress,
            policy_commitment(b"resolver-secret", policy_id.to_string().into_bytes())
                .expect("commitment"),
            resolver.public_key().clone(),
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        RegisterIdentifierPolicy { policy }
            .execute(&owner, &mut tx)
            .expect("register policy");
        ActivateIdentifierPolicy {
            policy_id: policy_id.clone(),
        }
        .execute(&owner, &mut tx)
        .expect("activate policy");
        let receipt = claim_receipt(
            &policy_id,
            &resolver,
            OpaqueAccountId::from(Hash::new(b"alice@example.com")),
            UniversalAccountId::from_hash(Hash::new(b"uaid-missing")),
            &owner,
            0,
            None,
        );
        let err = ClaimIdentifier {
            account: owner.clone(),
            receipt,
        }
        .execute(&owner, &mut tx)
        .expect_err("accounts without a UAID must be rejected");

        assert!(
            err.to_string().contains("does not have a UAID"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn claim_identifier_rejects_invalid_receipt_signature() {
        let mut state = test_state();
        let domain_id: DomainId = "directory".parse().expect("domain id");
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-invalid-signature"));
        seed_domain(&mut state, &domain_id, &owner);
        seed_account_with_uaid(&mut state, &owner, &domain_id, uaid);

        let resolver = KeyPair::random();
        let wrong_resolver = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let policy = IdentifierPolicy::new(
            policy_id.clone(),
            owner.clone(),
            IdentifierNormalization::PhoneE164,
            policy_commitment(b"resolver-secret", policy_id.to_string().into_bytes())
                .expect("commitment"),
            resolver.public_key().clone(),
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        RegisterIdentifierPolicy { policy }
            .execute(&owner, &mut tx)
            .expect("register policy");
        ActivateIdentifierPolicy {
            policy_id: policy_id.clone(),
        }
        .execute(&owner, &mut tx)
        .expect("activate policy");

        let err = ClaimIdentifier {
            account: owner.clone(),
            receipt: claim_receipt(
                &policy_id,
                &wrong_resolver,
                OpaqueAccountId::from(Hash::new(b"+15551234567")),
                uaid,
                &owner,
                0,
                Some(60_000),
            ),
        }
        .execute(&owner, &mut tx)
        .expect_err("claim must reject a receipt signed by a different resolver");

        assert!(
            err.to_string().contains("signature is invalid"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn claim_identifier_rejects_expired_receipts() {
        let mut state = test_state();
        let domain_id: DomainId = "directory".parse().expect("domain id");
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-expired-receipt"));
        seed_domain(&mut state, &domain_id, &owner);
        seed_account_with_uaid(&mut state, &owner, &domain_id, uaid);

        let resolver = KeyPair::random();
        let policy_id: IdentifierPolicyId = "email#retail".parse().expect("policy id");
        let policy = IdentifierPolicy::new(
            policy_id.clone(),
            owner.clone(),
            IdentifierNormalization::EmailAddress,
            policy_commitment(b"resolver-secret", policy_id.to_string().into_bytes())
                .expect("commitment"),
            resolver.public_key().clone(),
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 11, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        RegisterIdentifierPolicy { policy }
            .execute(&owner, &mut tx)
            .expect("register policy");
        ActivateIdentifierPolicy {
            policy_id: policy_id.clone(),
        }
        .execute(&owner, &mut tx)
        .expect("activate policy");

        let err = ClaimIdentifier {
            account: owner.clone(),
            receipt: claim_receipt(
                &policy_id,
                &resolver,
                OpaqueAccountId::from(Hash::new(b"alice@example.com")),
                uaid,
                &owner,
                5,
                Some(10),
            ),
        }
        .execute(&owner, &mut tx)
        .expect_err("claim must reject a receipt that is already expired");

        assert!(
            err.to_string().contains("expired at or before block time"),
            "unexpected error: {err}"
        );
    }
}
