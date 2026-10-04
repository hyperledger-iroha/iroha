//! Native originals and refusal controls. Synthetic table fixtures grant no runtime admission.

use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{IntoKeyValue, Registrable, account::Account};
use iroha_primitives::json::Json;
use std::cell::Cell;

fn identity(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

#[test]
fn exact_counter_accounts_are_borrowed_and_changed_or_missing_originals_refuse() {
    let owner = identity(81);
    let reader = identity(82);
    let mut world = World::new();
    for id in [&owner, &reader] {
        let (key, value) = Account::new(id.clone()).build(&owner).into_key_value();
        world.accounts.insert(key, value);
    }
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    let captured = capture(
        &block,
        &WorldStateAccumulator::capture(&block).unwrap(),
        &budget,
    )
    .unwrap();
    let original = counter_account_original(&captured.snapshot, &block, &owner).unwrap();
    assert!(core::ptr::eq(
        original,
        block.accounts().get(&owner).unwrap()
    ));
    counter_account_original(&captured.snapshot, &block, &reader).unwrap();
    assert!(counter_account_original(&captured.snapshot, &block, &identity(83)).is_err());
    let mut changed = block.accounts().get(&owner).unwrap().clone();
    changed.metadata.insert(
        "private_counter_policy_v1".parse().unwrap(),
        Json::from("tail-substitution"),
    );
    block.accounts.insert(owner.clone(), changed);
    assert!(counter_account_original(&captured.snapshot, &block, &owner).is_err());
    // A changed policy authority cannot contaminate the untouched reader's original.
    counter_account_original(&captured.snapshot, &block, &reader).unwrap();
}

#[test]
fn native_counter_cut_refuses_global_genesis_and_unfunded_reads_before_consumer() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let authority = chain.genesis_account().clone();
    let called = Cell::new(false);
    let genesis = chain.committed(1);
    let funded = AllocationBudget::new(16 * 1024 * 1024);
    assert!(
        chain
            .state()
            .with_native_private_counter_accounts_v1(
                &genesis,
                &authority,
                &authority,
                &funded,
                |_, _, _| {
                    called.set(true);
                    Ok(())
                },
            )
            .is_err()
    );
    chain.commit_at(2_000, vec![]);
    let tip = chain.committed(2);
    for budget in [&funded, &AllocationBudget::new(0)] {
        assert!(
            chain
                .state()
                .with_native_private_counter_accounts_v1(
                    &tip,
                    &authority,
                    &authority,
                    budget,
                    |_, _, _| {
                        called.set(true);
                        Ok(())
                    },
                )
                .is_err()
        );
        assert_eq!(budget.reserved_bytes(), 0);
    }
    assert!(!called.get());
}

#[test]
fn oversized_native_account_original_refuses_even_when_its_world_hash_matches() {
    let owner = identity(84);
    let (id, mut value) = Account::new(owner.clone()).build(&owner).into_key_value();
    // Each canonical JSON string fits the native 1 MiB value ceiling, while three
    // independently valid values exceed the complete original-account 2 MiB bound.
    let payload_bytes = 768 * 1024;
    assert!(payload_bytes + 2 < iroha_primitives::json::MAX_JSON_BYTES);
    assert!(3 * payload_bytes > MAX_COUNTER_ACCOUNT_ORIGINAL_BYTES);
    for index in 0..3 {
        value.metadata.insert(
            format!("unrelated_large_metadata_{index}").parse().unwrap(),
            Json::new("x".repeat(payload_bytes)),
        );
    }
    let mut world = World::new();
    world.accounts.insert(id, value);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let block = world.block();
    let captured = capture(
        &block,
        &WorldStateAccumulator::capture(&block).unwrap(),
        &budget,
    )
    .unwrap();
    assert!(counter_account_original(&captured.snapshot, &block, &owner).is_err());
}
