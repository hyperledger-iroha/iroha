//! Ensure scheduler ready-queue heap vs per-wave sort produce identical outcomes.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::borrow::Cow;

use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_data_model::prelude::*;
use mv::storage::StorageReadOnly;

mod snapshots;

fn run_with_ready_heap(
    ready_heap: bool,
    txs: Vec<SignedTransaction>,
) -> (String, iroha_core::state::State) {
    // Build world: two accounts, one asset def, balances seeded
    let (alice_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let ad: AssetDefinition =
        AssetDefinition::new("coin#wonderland".parse().unwrap(), NumericSpec::default())
            .build(&alice_id);
    let acc_a = Account::new(alice_id.clone().to_account_id(domain_id.clone())).build(&alice_id);
    let acc_b = Account::new(bob_id.clone().to_account_id(domain_id)).build(&alice_id);
    // Seed asset balances
    let a_coin = AssetId::of(ad.id().clone(), alice_id.clone());
    let b_coin = AssetId::of(ad.id().clone(), bob_id.clone());
    let a0 = Asset::new(
        a_coin.clone(),
        iroha_primitives::numeric::Numeric::new(60, 0),
    );
    let b0 = Asset::new(
        b_coin.clone(),
        iroha_primitives::numeric::Numeric::new(10, 0),
    );
    let world = iroha_core::state::World::with_assets([domain], [acc_a, acc_b], [ad], [a0, b0], []);
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = iroha_core::query::store::LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let mut state = iroha_core::state::State::new(
        world,
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state = iroha_core::state::State::new(world, kura, query);

    // Configure scheduler knob
    let mut cfg = state.view().pipeline().clone();
    cfg.ready_queue_heap = ready_heap;
    state.set_pipeline(cfg);

    // Build and execute block
    let block: SignedBlock = {
        let accepted: Vec<_> = txs
            .into_iter()
            .map(|t| iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(t)))
            .collect();
        BlockBuilder::new(accepted)
            .chain(0, state.view().latest_block().as_deref())
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key())
            .unpack(|_| {})
            .into()
    };
    let json = {
        let mut sb = state.block(block.header());
        let vb = ValidBlock::validate_unchecked(block, &mut sb).unpack(|_| {});
        let cb = vb.commit_unchecked().unpack(|_| {});
        let events = sb.apply_without_execution(&cb, Vec::new());
        snapshots::events_json_filtered(&events)
    };
    (json, state)
}

#[test]
fn scheduler_ready_queue_heap_vs_wave_sort_parity() {
    let chain_id = ChainId::from("chain");
    let (alice_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let rose: AssetDefinitionId = "coin#wonderland".parse().unwrap();
    let a_coin = AssetId::of(rose.clone(), alice_id.clone());
    let b_coin = AssetId::of(rose.clone(), bob_id.clone());

    // Build a set of independent txs so scheduler ordering/tie-breakers apply
    let txs = vec![
        TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([Mint::asset_numeric(5_u32, a_coin.clone())])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key()),
        TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([Transfer::asset_numeric(
                a_coin.clone(),
                3_u32,
                bob_id.clone(),
            )])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key()),
        TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([Burn::asset_numeric(1_u32, b_coin.clone())])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key()),
        TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([SetKeyValue::account(
                alice_id.clone(),
                "k".parse().unwrap(),
                iroha_primitives::json::Json::new("v"),
            )])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key()),
    ];

    let (json_heap, state_heap) = run_with_ready_heap(true, txs.clone());
    let (json_wave, state_wave) = run_with_ready_heap(false, txs);

    assert_eq!(json_heap, json_wave, "event sequences must match");
    let bal = |state: &iroha_core::state::State, id: &AssetId| {
        state.view().world().assets().get(id).map_or_else(
            || iroha_primitives::numeric::Numeric::new(0, 0),
            |v| v.clone().into_inner(),
        )
    };
    assert_eq!(bal(&state_heap, &a_coin), bal(&state_wave, &a_coin));
    assert_eq!(bal(&state_heap, &b_coin), bal(&state_wave, &b_coin));
}
