//! Canonical execution retains one owner for either parallel-apply setting.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//!
//! Actual published transactions must not report work from the retired detached DAG.
use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    governance::manifest::LaneManifestRegistry,
    state::StateReadOnly,
};
use iroha_data_model::prelude::*;
use iroha_model_base::chain::ChainId;
use iroha_model_base::domain::DomainId;
use std::{borrow::Cow, sync::Arc};
fn build_world() -> (
    iroha_core::state::State,
    NetworkId,
    AccountId,
    iroha_crypto::KeyPair,
) {
    let chain_id = ChainId::from("chain");
    let (alice_id, alice_kp) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let ad: AssetDefinition = AssetDefinition::new(
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        ),
        "coin".to_owned(),
        NumericSpec::default(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&alice_id);
    let acc_a = Account::new(alice_id.clone()).build(&alice_id);
    let world = iroha_core::state::World::with([domain], [acc_a], [ad]);
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = iroha_core::query::store::LiveQueryStore::start_test();
    let state =
        iroha_core::state::State::new_with_chain_for_testing(world, kura, query, chain_id.clone());
    let network_id = *state.network_id_ref();
    let nexus = state.nexus_snapshot();
    state.install_lane_manifests(&Arc::new(
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
    ));
    (state, network_id, alice_id, alice_kp)
}
fn make_block(
    state: &iroha_core::state::State,
    network_id: &NetworkId,
    alice_id: &AccountId,
    kp: &iroha_crypto::KeyPair,
) -> iroha_data_model::block::SignedBlock {
    let genesis = state
        .seed_signed_genesis_for_testing(&iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR)
        .expect("publish fixture genesis");
    // Two simple instructions to ensure a non-empty overlay
    let asset = AssetId::of(
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        ),
        alice_id.clone(),
    );
    let instrs: Vec<InstructionBox> = vec![
        Mint::asset_quantity(5_u32, asset).into(),
        SetKeyValue::account(
            alice_id.clone(),
            "k".parse().unwrap(),
            iroha_primitives::json::Json::new("v"),
        )
        .into(),
    ];
    let tx = TransactionBuilder::new(
        *network_id,
        alice_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instrs)
    .sign(kp.private_key());
    let accepted = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    BlockBuilder::new(vec![accepted])
        .chain(0, Some(&genesis))
        .sign(kp.private_key())
        .unpack(|_| {})
        .into()
}
#[test]
#[cfg(feature = "telemetry")]
fn canonical_output_owner_does_not_allocate_detached_journals() {
    // Sequential mode: expect detached counters to be zero
    let (mut state_seq, chain_id, alice_id, kp) = build_world();
    let mut cfg = state_seq.view().pipeline().clone();
    cfg.parallel_apply = false;
    state_seq.set_pipeline(cfg);
    let new_block = make_block(&state_seq, &chain_id, &alice_id, &kp);
    let mut sb = state_seq.block(new_block.header());
    let vb = ValidBlock::validate_unchecked(new_block, &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    assert_eq!(cb.as_ref().output_results().count(), 1);
    assert!(cb.as_ref().output_error(0).is_none());
    state_seq
        .commit_executed_block_for_testing(sb, cb)
        .expect("publish canonical work for the configured apply setting");
    let (prep_s, merged_s, fallback_s) = state_seq.view().metrics().pipeline_detached_counts();
    let status_s = iroha_core::sumeragi::status::snapshot();
    assert_eq!(prep_s, 0, "sequential: prepared must be zero");
    assert_eq!(merged_s, 0, "sequential: merged must be zero");
    assert_eq!(fallback_s, 0, "sequential: fallback must be zero");
    assert_eq!(
        status_s.pipeline_execution.detached_prepared_total, 0,
        "sequential status: prepared must be zero"
    );
    // The parallel setting must retain the same canonical execution owner.
    let (mut state_par, chain_id, alice_id, kp) = build_world();
    let mut cfg = state_par.view().pipeline().clone();
    cfg.parallel_apply = true;
    state_par.set_pipeline(cfg);
    let new_block = make_block(&state_par, &chain_id, &alice_id, &kp);
    let mut sb = state_par.block(new_block.header());
    let vb = ValidBlock::validate_unchecked(new_block, &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    assert_eq!(cb.as_ref().output_results().count(), 1);
    assert!(cb.as_ref().output_error(0).is_none());
    state_par
        .commit_executed_block_for_testing(sb, cb)
        .expect("publish canonical work for the configured apply setting");
    let (prep_p, _merged_p, _fallback_p) = state_par.view().metrics().pipeline_detached_counts();
    let status_p = iroha_core::sumeragi::status::snapshot();
    assert_eq!(prep_p, 0, "canonical owner retains its original journal");
    assert_eq!(status_p.pipeline_execution.detached_prepared_total, 0);
}
#[test]
#[cfg(not(feature = "telemetry"))]
fn parallel_apply_knob_compiles_without_telemetry() {
    // Smoke test: ensure code path compiles and runs without telemetry; no metrics assertions.
    for &flag in &[false, true] {
        let (mut state, chain_id, alice_id, kp) = build_world();
        let mut cfg = state.view().pipeline().clone();
        cfg.parallel_apply = flag;
        state.set_pipeline(cfg);
        let new_block = make_block(&state, &chain_id, &alice_id, &kp);
        let mut sb = state.block(new_block.header());
        let vb = ValidBlock::validate_unchecked(new_block, &mut sb).unpack(|_| {});
        let cb = vb.commit_unchecked().unpack(|_| {});
        assert_eq!(cb.as_ref().output_results().count(), 1);
        assert!(cb.as_ref().output_error(0).is_none());
        state
            .commit_executed_block_for_testing(sb, cb)
            .expect("publish canonical work for the configured apply setting");
        let status = iroha_core::sumeragi::status::snapshot();
        assert_eq!(
            status.pipeline_execution.detached_prepared_total, 0,
            "canonical execution retains its original journal for either setting"
        );
    }
}
