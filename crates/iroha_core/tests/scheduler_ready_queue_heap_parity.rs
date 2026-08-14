//! Ensure scheduler ready-queue heap vs per-wave sort produce identical outcomes.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    governance::manifest::LaneManifestRegistry,
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_data_model::prelude::*;
use mv::storage::StorageReadOnly;
use std::{borrow::Cow, sync::Arc};
mod snapshots;
fn test_network_id(label: &[u8]) -> NetworkId {
    NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::new(label),
        ),
    )
}
fn run_with_ready_heap(
    ready_heap: bool,
    network_id: &NetworkId,
    txs: Vec<SignedTransaction>,
    alice_id: &AccountId,
    bob_id: &AccountId,
) -> (String, iroha_core::state::State) {
    // Build world: two accounts, one asset def, balances seeded
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(alice_id);
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
    .build(alice_id);
    let acc_a = Account::new(alice_id.clone()).build(alice_id);
    let acc_b = Account::new(bob_id.clone()).build(alice_id);
    // Seed asset balances
    let a_coin = AssetId::of(ad.id().clone(), alice_id.clone());
    let b_coin = AssetId::of(ad.id().clone(), bob_id.clone());
    let a0 = Asset::new(a_coin.clone(), Quantity::from(60_u64));
    let b0 = Asset::new(b_coin.clone(), Quantity::from(10_u64));
    let world = iroha_core::state::World::with_assets([domain], [acc_a, acc_b], [ad], [a0, b0], []);
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = iroha_core::query::store::LiveQueryStore::start_test();
    let mut state = iroha_core::state::State::new_with_chain_and_network_id_for_testing(
        world,
        kura,
        query,
        ChainId::from("chain"),
        *network_id,
    );
    let nexus = state.nexus_snapshot();
    state.install_lane_manifests(&Arc::new(
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
    ));
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
    let network_id = test_network_id(b"scheduler-ready-queue-heap-parity");
    let (alice_id, alice_keypair) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let rose: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
    let a_coin = AssetId::of(rose.clone(), alice_id.clone());
    let b_coin = AssetId::of(rose.clone(), bob_id.clone());
    // Build a set of independent txs so scheduler ordering/tie-breakers apply
    let txs = vec![
        TransactionBuilder::new(
            network_id,
            alice_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Mint::asset_quantity(5_u32, a_coin.clone())])
        .sign(alice_keypair.private_key()),
        TransactionBuilder::new(
            network_id,
            alice_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Transfer::asset_quantity(
            a_coin.clone(),
            3_u32,
            bob_id.clone(),
        )])
        .sign(alice_keypair.private_key()),
        TransactionBuilder::new(
            network_id,
            alice_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Burn::asset_quantity(1_u32, b_coin.clone())])
        .sign(alice_keypair.private_key()),
        TransactionBuilder::new(
            network_id,
            alice_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([SetKeyValue::account(
            alice_id.clone(),
            "k".parse().unwrap(),
            iroha_primitives::json::Json::new("v"),
        )])
        .sign(alice_keypair.private_key()),
    ];
    let (json_heap, state_heap) =
        run_with_ready_heap(true, &network_id, txs.clone(), &alice_id, &bob_id);
    let (json_wave, state_wave) = run_with_ready_heap(false, &network_id, txs, &alice_id, &bob_id);
    assert_eq!(json_heap, json_wave, "event sequences must match");
    let bal = |state: &iroha_core::state::State, id: &AssetId| {
        state
            .view()
            .world()
            .assets()
            .get(id)
            .map_or_else(Quantity::zero, |v| v.clone().into_inner())
    };
    assert_eq!(bal(&state_heap, &a_coin), bal(&state_wave, &a_coin));
    assert_eq!(bal(&state_heap, &b_coin), bal(&state_wave, &b_coin));
}
