//! Regression tests for account asset lookup queries.

use crate::native_genesis::certified_state;
use iroha_core::{
    smartcontracts::{Execute, ValidQuery},
    state::{World, WorldReadOnly},
};
use iroha_data_model::{prelude::*, query::dsl::CompoundPredicate};
use iroha_model_base::domain::DomainId;
use iroha_primitives::nonzero;
use iroha_test_samples::{ALICE_ID, gen_account_in};
use mv::storage::StorageReadOnly;
use std::collections::BTreeSet;
#[test]
fn multi_account_mint_returns_only_positive_holders() {
    // The domain is part of the authenticated genesis World; live domain registration
    // requires an SNS lease, which is not the subject of this holder-index test.
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain");
    let world = World::with(
        [Domain::new(domain_id.clone()).build(&ALICE_ID)],
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
    );
    let state = certified_state(world);
    // The first ordinary block after the authenticated genesis.
    let header = BlockHeader::new(
        nonzero!(2_u64),
        state.view().latest_block_hash(),
        None,
        1,
        0,
    );
    let mut state_block = state.block(header);
    let mut stx = state_block.transaction();
    let (holder_a, _kp_a) = gen_account_in("wonderland");
    let (holder_b, _kp_b) = gen_account_in("wonderland");
    let (zero_holder, _kp_zero) = gen_account_in("wonderland");
    let (untouched, _kp_untouched) = gen_account_in("wonderland");
    for account_id in [&holder_a, &holder_b, &zero_holder, &untouched] {
        Register::account(Account::new(account_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register account");
    }
    let definition_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "multi_coin".parse().unwrap(),
        );
    Register::asset_definition(AssetDefinition::numeric(
        definition_id.clone(),
        "multi_coin".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        Some(domain_id),
    ))
    .execute(&ALICE_ID, &mut stx)
    .expect("register asset definition");
    Mint::asset_quantity(5u32, AssetId::new(definition_id.clone(), holder_a.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("mint to holder A");
    Mint::asset_quantity(7u32, AssetId::new(definition_id.clone(), holder_b.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("mint to holder B");
    let zero_asset_id = AssetId::new(definition_id.clone(), zero_holder.clone());
    let error = Mint::asset_quantity(Quantity::zero(), zero_asset_id.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect_err("zero mint must be rejected");
    assert_eq!(
        error,
        iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(
            "asset mint amount must be non-zero".into(),
        ),
    );
    assert!(stx.world.assets().get(&zero_asset_id).is_none());
    // Exercise the holder index through valid balance changes, including removal at zero.
    Mint::asset_quantity(1u32, zero_asset_id.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("mint to the holder whose balance will return to zero");
    assert!(
        FindAccountsWithAsset::new(definition_id.clone())
            .execute(CompoundPredicate::PASS, &stx)
            .expect("query after mint succeeds")
            .any(|account| account.id() == &zero_holder),
        "a positive balance must add the account to the holder index"
    );
    Burn::asset_quantity(1u32, zero_asset_id.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("burn the holder's full balance");
    assert!(stx.world.assets().get(&zero_asset_id).is_none());
    stx.apply();
    state_block.commit_world_overlay_for_testing().unwrap();
    let expected: BTreeSet<_> = [holder_a.clone(), holder_b.clone()].into_iter().collect();
    let view = state.view();
    let actual: BTreeSet<_> = FindAccountsWithAsset::new(definition_id)
        .execute(CompoundPredicate::PASS, &view)
        .expect("query succeeds")
        .map(|account| account.id)
        .collect();
    assert_eq!(actual, expected, "only positive holders should be returned");
    assert!(!actual.contains(&zero_holder));
    assert!(!actual.contains(&untouched));
}
