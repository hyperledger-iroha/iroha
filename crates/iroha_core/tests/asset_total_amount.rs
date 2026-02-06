//! Validates that aggregate asset totals stay aligned with per-account balances.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use core::num::NonZeroU64;

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
#[allow(clippy::too_many_lines)]
fn asset_totals_track_multi_account_mint_and_burn() {
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

    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut state_block = state.block(header);
    let mut stx = state_block.transaction();

    let wonderland: DomainId = "wonderland".parse().expect("domain");
    let looking_glass: DomainId = "looking_glass".parse().expect("domain");

    Register::domain(Domain::new(wonderland.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("register wonderland");
    Register::domain(Domain::new(looking_glass.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("register looking_glass");

    let (holder_wonderland, _kp_a) = gen_account_in("wonderland");
    let (holder_looking_glass, _kp_b) = gen_account_in("looking_glass");
    let (burn_to_zero, _kp_c) = gen_account_in("looking_glass");

    for account_id in [
        holder_wonderland.clone(),
        holder_looking_glass.clone(),
        burn_to_zero.clone(),
    ] {
        Register::account(Account::new(account_id))
            .execute(&ALICE_ID, &mut stx)
            .expect("register account");
    }

    let definition_id: AssetDefinitionId = "multi_total#wonderland".parse().expect("asset def");
    Register::asset_definition(AssetDefinition::numeric(definition_id.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("register asset definition");

    Mint::asset_numeric(
        50_u32,
        AssetId::new(definition_id.clone(), holder_wonderland.clone()),
    )
    .execute(&ALICE_ID, &mut stx)
    .expect("mint wonderland holder");
    Mint::asset_numeric(
        40_u32,
        AssetId::new(definition_id.clone(), holder_looking_glass.clone()),
    )
    .execute(&ALICE_ID, &mut stx)
    .expect("mint looking glass holder");
    Mint::asset_numeric(
        10_u32,
        AssetId::new(definition_id.clone(), burn_to_zero.clone()),
    )
    .execute(&ALICE_ID, &mut stx)
    .expect("mint burn-to-zero holder");

    Burn::asset_numeric(
        15_u32,
        AssetId::new(definition_id.clone(), holder_wonderland.clone()),
    )
    .execute(&ALICE_ID, &mut stx)
    .expect("partial burn from wonderland holder");
    Burn::asset_numeric(
        5_u32,
        AssetId::new(definition_id.clone(), holder_looking_glass.clone()),
    )
    .execute(&ALICE_ID, &mut stx)
    .expect("partial burn from looking glass holder");
    Burn::asset_numeric(
        10_u32,
        AssetId::new(definition_id.clone(), burn_to_zero.clone()),
    )
    .execute(&ALICE_ID, &mut stx)
    .expect("burn to zero");

    stx.apply();
    state_block.commit().unwrap();

    let view = state.view();
    let definition = FindAssetsDefinitions::new()
        .execute(CompoundPredicate::PASS, &view)
        .expect("query asset definitions")
        .find(|definition| definition.id() == &definition_id)
        .expect("definition present");
    let definition_total = definition.total_quantity();

    let mut manual_total = Numeric::zero();
    for asset in FindAssets::new()
        .execute(CompoundPredicate::PASS, &view)
        .expect("query assets")
        .filter(|asset| asset.id().definition() == &definition_id)
    {
        manual_total = manual_total
            .checked_add(asset.value().clone())
            .expect("manual total should not overflow");
    }

    assert_eq!(manual_total, definition_total.clone());
    assert_eq!(manual_total, numeric!(70));
}
