//! Tests for batched transfer transcripts.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::prelude::*;
use iroha_primitives::numeric::Numeric;
use iroha_test_samples::{ALICE_ID, gen_account_in};
use nonzero_ext::nonzero;

#[test]
fn transfer_asset_batch_records_multi_delta_transcript() {
    let domain_id: DomainId = "wonderland".parse().expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);

    let (bob_id, _) = gen_account_in("wonderland");
    let (carol_id, _) = gen_account_in("wonderland");
    let alice_account =
        Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let bob_account = Account::new(bob_id.clone()).build(&ALICE_ID);
    let carol_account = Account::new(carol_id.clone()).build(&ALICE_ID);

    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "rose".parse().unwrap(),
    );
    let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);
    let alice_asset = Asset::new(
        AssetId::new(asset_def_id.clone(), ALICE_ID.clone()),
        Numeric::from(100_u32),
    );

    let world = World::with_assets(
        [domain],
        [alice_account, bob_account, carol_account],
        [asset_def],
        [alice_asset],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_store = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query_store);

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    tx.tx_call_hash = Some(Hash::prehashed([0xAB; Hash::LENGTH]));

    let batch = TransferAssetBatch::new(vec![
        TransferAssetBatchEntry::new(
            ALICE_ID.clone(),
            bob_id.clone(),
            asset_def_id.clone(),
            Numeric::from(10_u32),
        ),
        TransferAssetBatchEntry::new(
            ALICE_ID.clone(),
            carol_id.clone(),
            asset_def_id.clone(),
            Numeric::from(5_u32),
        ),
    ]);

    batch
        .execute(&ALICE_ID, &mut tx)
        .expect("batch executes successfully");
    tx.apply();

    let transcripts = block.drain_transfer_transcripts();
    assert_eq!(transcripts.len(), 1);
    let entry = transcripts
        .values()
        .next()
        .expect("transcript recorded for batch");
    assert_eq!(entry.len(), 1, "single transcript for batch");
    assert_eq!(entry[0].deltas.len(), 2, "two deltas recorded");
    assert!(entry[0].poseidon_preimage_digest.is_none());

    let world = &block.world;
    let alice_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
    let bob_asset_id = AssetId::new(asset_def_id.clone(), bob_id);
    let carol_asset_id = AssetId::new(asset_def_id, carol_id);

    let alice_balance = world.asset(&alice_asset_id).expect("alice asset");
    assert_eq!(**alice_balance, Numeric::from(85_u32));
    let bob_balance = world.asset(&bob_asset_id).expect("bob asset");
    assert_eq!(**bob_balance, Numeric::from(10_u32));
    let carol_balance = world.asset(&carol_asset_id).expect("carol asset");
    assert_eq!(**carol_balance, Numeric::from(5_u32));
}
