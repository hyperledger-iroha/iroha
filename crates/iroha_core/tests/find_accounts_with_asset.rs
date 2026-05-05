//! Regression tests for account asset lookup queries.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use core::num::NonZeroU64;
use std::collections::BTreeSet;

#[cfg(feature = "telemetry")]
use iroha_core::telemetry::StateTelemetry;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::{Execute, ValidQuery},
    state::{State, World},
};
use iroha_data_model::{prelude::*, query::dsl::CompoundPredicate};
use iroha_primitives::numeric::Numeric;
use iroha_test_samples::{ALICE_ID, gen_account_in};

#[test]
fn multi_account_mint_returns_only_positive_holders() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let telemetry = StateTelemetry::default();
    let state = State::new(
        World::default(),
        kura,
        query_handle,
        #[cfg(feature = "telemetry")]
        telemetry,
    );

    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut state_block = state.block(header);
    let mut stx = state_block.transaction();

    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain");
    Register::domain(Domain::new(domain_id.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("register domain");

    let (holder_a, _kp_a) = gen_account_in("wonderland");
    let (holder_b, _kp_b) = gen_account_in("wonderland");
    let (zero_holder, _kp_zero) = gen_account_in("wonderland");
    let (untouched, _kp_untouched) = gen_account_in("wonderland");

    for account_id in [&holder_a, &holder_b, &zero_holder, &untouched] {
        Register::account(Account::new(account_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register account");
    }

    let definition_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "multi_coin".parse().unwrap(),
    );
    Register::asset_definition(
        AssetDefinition::numeric(definition_id.clone()).with_name(definition_id.name().to_string()),
    )
    .execute(&ALICE_ID, &mut stx)
    .expect("register asset definition");

    Mint::asset_numeric(5u32, AssetId::new(definition_id.clone(), holder_a.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("mint to holder A");
    Mint::asset_numeric(7u32, AssetId::new(definition_id.clone(), holder_b.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("mint to holder B");
    Mint::asset_numeric(
        Numeric::zero(),
        AssetId::new(definition_id.clone(), zero_holder.clone()),
    )
    .execute(&ALICE_ID, &mut stx)
    .expect("zero mint succeeds");

    stx.apply();
    state_block.commit().unwrap();

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
