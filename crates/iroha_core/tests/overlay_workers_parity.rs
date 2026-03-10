//! Ensure overlay construction with different `pipeline.workers` settings yields
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! identical outcomes (events and final state), preserving determinism.

use std::borrow::Cow;

use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_data_model::prelude::*;
use mv::storage::StorageReadOnly; // trait for .get()

mod snapshots;

fn run_with_workers(
    workers: usize,
    txs: Vec<SignedTransaction>,
) -> (String, iroha_core::state::State) {
    // Build a fresh world with a domain, two accounts, and a numeric asset definition
    let (alice_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let ad: AssetDefinition =
        AssetDefinition::new("coin#wonderland".parse().unwrap(), NumericSpec::default())
            .build(&alice_id);
    let acc_a = Account::new(alice_id.clone().to_account_id(domain_id.clone())).build(&alice_id);
    let acc_b = Account::new(bob_id.clone().to_account_id(domain_id)).build(&alice_id);
    let world = iroha_core::state::World::with([domain], [acc_a, acc_b], [ad]);
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
    // Configure overlay parallelism and fixed worker pool size
    let mut cfg = state.view().pipeline().clone();
    cfg.parallel_overlay = true;
    cfg.workers = workers; // 0 = Rayon default
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
    let mut sb = state.block(block.header());
    let vb = ValidBlock::validate_unchecked(block, &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    let events = sb.apply_without_execution(&cb, Vec::new());
    let json = snapshots::events_json_filtered(&events);
    // Ensure StateBlock borrow ends before returning the state
    drop(sb);
    (json, state)
}

#[test]
fn overlay_parallel_workers_parity() {
    let chain_id = ChainId::from("chain");
    let (alice_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let rose: AssetDefinitionId = "coin#wonderland".parse().unwrap();
    let a_coin = AssetId::of(rose.clone(), alice_id.clone());
    let b_coin = AssetId::of(rose.clone(), bob_id.clone());

    // Build a mixed set of instruction-only transactions to exercise overlay builder
    let txs: Vec<SignedTransaction> = vec![
        TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([SetKeyValue::account(
                alice_id.clone(),
                "k1".parse().unwrap(),
                iroha_primitives::json::Json::new("v1"),
            )])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key()),
        TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([SetKeyValue::domain(
                "wonderland".parse().unwrap(),
                "dk".parse().unwrap(),
                iroha_primitives::json::Json::new(3u32),
            )])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key()),
        TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([Mint::asset_numeric(7_u32, a_coin.clone())])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key()),
        TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([Burn::asset_numeric(2_u32, b_coin.clone())])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key()),
        TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([Transfer::asset_numeric(
                a_coin.clone(),
                5_u32,
                bob_id.clone(),
            )])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key()),
    ];

    // Run with workers=0 (Rayon default) and workers=2
    let (json0, state0) = run_with_workers(0, txs.clone());
    let (json2, state2) = run_with_workers(2, txs);

    // Compare event JSON and balances
    assert_eq!(
        json0, json2,
        "events must be identical across worker settings"
    );
    let bal = |state: &iroha_core::state::State, id: &AssetId| {
        state.view().world().assets().get(id).map_or_else(
            || iroha_primitives::numeric::Numeric::new(0, 0),
            |v| v.clone().into_inner(),
        )
    };
    assert_eq!(bal(&state0, &a_coin), bal(&state2, &a_coin));
    assert_eq!(bal(&state0, &b_coin), bal(&state2, &b_coin));
}
