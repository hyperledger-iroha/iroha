//! GPU key-bucket parity: enabling `pipeline.gpu_key_bucket` must not change
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! scheduling outcomes or final state. This toggles the knob and compares events
//! and balances for a mixed set of transactions.

use std::borrow::Cow;

use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_data_model::prelude::*;
use mv::storage::StorageReadOnly; // trait for .get()

mod snapshots;

fn run_with_gpu_bucket(
    gpu_key_bucket: bool,
    txs: Vec<SignedTransaction>,
) -> (String, iroha_core::state::State) {
    // Build a fresh world with a domain, two accounts, and a numeric asset definition
    let (alice_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let ad: AssetDefinition = AssetDefinition::new(
        iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "coin".parse().unwrap(),
        ),
        NumericSpec::default(),
    )
    .build(&alice_id);
    let acc_a = Account::new(alice_id.clone()).build(&alice_id);
    let acc_b = Account::new(bob_id.clone()).build(&alice_id);
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
    // Toggle GPU key-bucketing knob
    let mut cfg = state.view().pipeline().clone();
    cfg.gpu_key_bucket = gpu_key_bucket;
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
fn scheduler_gpu_key_bucket_parity() {
    let chain_id = ChainId::from("chain");
    let (alice_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let rose: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "coin".parse().unwrap(),
    );
    let a_coin = AssetId::of(rose.clone(), alice_id.clone());
    let b_coin = AssetId::of(rose.clone(), bob_id.clone());

    // Mixed instruction set to exercise scheduler prepass and DSU unions
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
        // Mint/Transfer on same asset to induce a dependency edge
        TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([Mint::asset_numeric(7_u32, a_coin.clone())])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key()),
        TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([Transfer::asset_numeric(
                a_coin.clone(),
                5_u32,
                bob_id.clone(),
            )])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key()),
        // Burn on bob to touch a different key
        TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([Burn::asset_numeric(2_u32, b_coin.clone())])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key()),
    ];

    // Compare with gpu_key_bucket OFF vs ON
    let (json_off, state_off) = run_with_gpu_bucket(false, txs.clone());
    let (json_on, state_on) = run_with_gpu_bucket(true, txs);

    assert_eq!(
        json_off, json_on,
        "events must match with/without gpu_key_bucket"
    );
    let bal = |state: &iroha_core::state::State, id: &AssetId| {
        state.view().world().assets().get(id).map_or_else(
            || iroha_primitives::numeric::Numeric::new(0, 0),
            |v| v.clone().into_inner(),
        )
    };
    assert_eq!(bal(&state_off, &a_coin), bal(&state_on, &a_coin));
    assert_eq!(bal(&state_off, &b_coin), bal(&state_on, &b_coin));
}
