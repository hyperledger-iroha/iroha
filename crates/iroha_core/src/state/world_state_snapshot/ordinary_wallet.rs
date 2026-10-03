//! Same actual account S/W original publication at one qualified certified World cut.
use super::*;
use iroha_crypto::{Algorithm, PublicKey};
use iroha_data_model::account::AccountValue;
impl State {
    /// Borrow only registered S/W values whose exact hashes occur in the original certified cut.
    /// The signed HTTP corridor has already authenticated the actual W request under S. This
    /// method rechecks the same scope under the held World overlay; no ledger-wide grant is used.
    /// # Errors
    /// Refuses missing/currently revoked registration, unsupported W controller, changed cut,
    /// post-tail modified rows or finite capture/serialization budget.
    #[allow(clippy::too_many_arguments)]
    pub fn with_native_ordinary_wallet_current_v1<T>(
        &self,
        tip: &CommittedBlock,
        authenticated_wallet: &AccountId,
        authenticated_signer: &PublicKey,
        signatory: &AccountId,
        wallet: &AccountId,
        budget: &AllocationBudget,
        consume: impl FnOnce(&WorldStateSnapshotV1, &AccountValue, &AccountValue) -> Result<T, String>,
    ) -> Result<T, WorldStateSnapshotError> {
        let key = signatory
            .try_signatory()
            .ok_or("ordinary current S is not a signatory")?;
        let policy = wallet
            .multisig_policy()
            .ok_or("ordinary current W is not multisig")?;
        if authenticated_wallet != wallet
            || authenticated_signer != key
            || key.algorithm() != Algorithm::Ed25519
            || policy.threshold() != 1
            || policy.members().len() != 1
            || policy.members()[0].weight() != 1
            || policy.members()[0].public_key() != key
            || signatory == wallet
        {
            return Err("ordinary current authenticated S/W scope differs".into());
        }
        self.with_native_world_state_snapshot_cut_v1(tip, None, budget, |snapshot, world| {
            let s = world
                .accounts()
                .get(signatory)
                .ok_or("ordinary current S registration is absent")?;
            let w = world
                .accounts()
                .get(wallet)
                .ok_or("ordinary current W registration is absent")?;
            for (id, value) in [(signatory, s), (wallet, w)] {
                require_target(
                    snapshot,
                    "world.accounts",
                    WorldStateElementKindV1::Table,
                    Some(hash_value(id)?),
                    hash_value(value)?,
                )?;
            }
            consume(snapshot, s, w)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        state::World,
        sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
    };
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        account::{Account, MultisigMember, MultisigPolicy},
        isi::Register,
    };
    use std::cell::Cell;
    fn fixture() -> (CertifiedTestChain, KeyPair, AccountId, AccountId) {
        let key = KeyPair::from_seed(vec![13; 32], Algorithm::Ed25519);
        let s = AccountId::new(key.public_key().clone());
        let w = AccountId::new_multisig(
            MultisigPolicy::new(
                1,
                vec![MultisigMember::new(key.public_key().clone(), 1).unwrap()],
            )
            .unwrap(),
        );
        let mut config = TestChainConfig::new(World::new(), 1_000);
        config.genesis_instructions = vec![
            Register::account(Account::new(s.clone())).into(),
            Register::account(Account::new(w.clone())).into(),
        ];
        let mut chain = CertifiedTestChain::start(config).unwrap();
        chain.commit_at(2_000, vec![]);
        (chain, key, s, w)
    }
    #[test]
    fn ordinary_wallet_cut_borrows_real_registered_scope_without_ledger_reader_permission() {
        let (chain, key, s, w) = fixture();
        let tip = chain.committed(2);
        let budget = AllocationBudget::new(16 * 1024 * 1024);
        chain
            .state()
            .with_native_ordinary_wallet_current_v1(
                &tip,
                &w,
                key.public_key(),
                &s,
                &w,
                &budget,
                |snapshot, sv, wv| {
                    assert_eq!(
                        snapshot.root().unwrap(),
                        tip.commitment().execution.world_state_root
                    );
                    assert_eq!(
                        snapshot.schema_hash,
                        State::native_world_schema_hash_v1().unwrap()
                    );
                    require_target(
                        snapshot,
                        "world.accounts",
                        WorldStateElementKindV1::Table,
                        Some(hash_value(&s)?),
                        hash_value(sv)?,
                    )?;
                    require_target(
                        snapshot,
                        "world.accounts",
                        WorldStateElementKindV1::Table,
                        Some(hash_value(&w)?),
                        hash_value(wv)?,
                    )?;
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(budget.reserved_bytes(), 0);
    }
    #[test]
    fn ordinary_wallet_cut_refuses_other_signer_unfunded_and_obsolete_cut_before_consumer() {
        let (mut chain, key, s, w) = fixture();
        let tip = chain.committed(2);
        let called = Cell::new(false);
        let other = KeyPair::from_seed(vec![14; 32], Algorithm::Ed25519);
        for (signer, bytes) in [
            (other.public_key(), 16 * 1024 * 1024),
            (key.public_key(), 0),
        ] {
            let budget = AllocationBudget::new(bytes);
            assert!(
                chain
                    .state()
                    .with_native_ordinary_wallet_current_v1(
                        &tip,
                        &w,
                        signer,
                        &s,
                        &w,
                        &budget,
                        |_, _, _| {
                            called.set(true);
                            Ok(())
                        }
                    )
                    .is_err()
            );
            assert_eq!(budget.reserved_bytes(), 0);
        }
        chain.commit_at(3_000, vec![]);
        assert!(
            chain
                .state()
                .with_native_ordinary_wallet_current_v1(
                    &tip,
                    &w,
                    key.public_key(),
                    &s,
                    &w,
                    &AllocationBudget::new(16 * 1024 * 1024),
                    |_, _, _| {
                        called.set(true);
                        Ok(())
                    }
                )
                .is_err()
        );
        assert!(!called.get());
    }
}
