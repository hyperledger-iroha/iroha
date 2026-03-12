//! Verify `pipeline.parallel_apply` knob is honored by the block executor.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//!
//! We run the same block twice with `parallel_apply=false` and `parallel_apply=true`.
//! With telemetry enabled, the detached-pipeline counters should remain zero in
//! sequential mode and be non-zero in parallel mode (at least `prepared`).

use std::borrow::Cow;

use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    state::StateReadOnly,
};
use iroha_data_model::prelude::*;

fn build_world() -> (
    iroha_core::state::State,
    ChainId,
    AccountId,
    iroha_crypto::KeyPair,
) {
    let chain_id = ChainId::from("chain");
    let (alice_id, alice_kp) = iroha_test_samples::gen_account_in("wonderland");
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
    let acc_a = Account::new(alice_id.clone().to_account_id(domain_id)).build(&alice_id);
    let world = iroha_core::state::World::with([domain], [acc_a], [ad]);
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = iroha_core::query::store::LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = iroha_core::state::State::new(
        world,
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = iroha_core::state::State::new(world, kura, query);
    (state, chain_id, alice_id, alice_kp)
}

fn make_block(
    chain_id: &ChainId,
    alice_id: &AccountId,
    kp: &iroha_crypto::KeyPair,
) -> iroha_data_model::block::SignedBlock {
    // Two simple instructions to ensure a non-empty overlay
    let asset = AssetId::of(
        iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "coin".parse().unwrap(),
        ),
        alice_id.clone(),
    );
    let instrs: Vec<InstructionBox> = vec![
        Mint::asset_numeric(5_u32, asset).into(),
        SetKeyValue::account(
            alice_id.clone(),
            "k".parse().unwrap(),
            iroha_primitives::json::Json::new("v"),
        )
        .into(),
    ];
    let tx = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions(instrs)
        .sign(kp.private_key());
    let accepted = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    BlockBuilder::new(vec![accepted])
        .chain(0, None)
        .sign(kp.private_key())
        .unpack(|_| {})
        .into()
}

#[test]
#[cfg(feature = "telemetry")]
fn parallel_apply_knob_affects_detached_counters() {
    // Sequential mode: expect detached counters to be zero
    let (mut state_seq, chain_id, alice_id, kp) = build_world();
    let mut cfg = state_seq.view().pipeline().clone();
    cfg.parallel_apply = false;
    state_seq.set_pipeline(cfg);
    let new_block = make_block(&chain_id, &alice_id, &kp);
    let mut sb = state_seq.block(new_block.header());
    let vb = ValidBlock::validate_unchecked(new_block, &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    let _ = sb.apply_without_execution(&cb, Vec::new());
    let (prep_s, merged_s, fallback_s) = state_seq.view().metrics().pipeline_detached_counts();
    assert_eq!(prep_s, 0, "sequential: prepared must be zero");
    assert_eq!(merged_s, 0, "sequential: merged must be zero");
    assert_eq!(fallback_s, 0, "sequential: fallback must be zero");

    // Parallel mode: expect at least one prepared entry
    let (mut state_par, chain_id, alice_id, kp) = build_world();
    let mut cfg = state_par.view().pipeline().clone();
    cfg.parallel_apply = true;
    state_par.set_pipeline(cfg);
    let new_block = make_block(&chain_id, &alice_id, &kp);
    let mut sb = state_par.block(new_block.header());
    let vb = ValidBlock::validate_unchecked(new_block, &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    let _ = sb.apply_without_execution(&cb, Vec::new());
    let (prep_p, _merged_p, _fallback_p) = state_par.view().metrics().pipeline_detached_counts();
    assert!(
        prep_p >= 1,
        "parallel: expected at least one prepared entry"
    );
}

#[test]
#[cfg(not(feature = "telemetry"))]
fn parallel_apply_knob_compiles_without_telemetry() {
    // Smoke test: ensure code path compiles and runs without telemetry; no metrics assertions.
    let (mut state, chain_id, alice_id, kp) = build_world();
    for &flag in &[false, true] {
        let mut cfg = state.view().pipeline().clone();
        cfg.parallel_apply = flag;
        state.set_pipeline(cfg);
        let new_block = make_block(&chain_id, &alice_id, &kp);
        let mut sb = state.block(new_block.header());
        let vb = ValidBlock::validate_unchecked(new_block, &mut sb).unpack(|_| {});
        let cb = vb.commit_unchecked().unpack(|_| {});
        let _ = sb.apply_without_execution(&cb, Vec::new());
    }
}
