#![allow(missing_docs)]

use std::str::FromStr as _;

use byte_unit::Byte;
use criterion::{criterion_group, criterion_main, Criterion};
use iroha_config::parameters::actual::Kura as Config;
use iroha_core::{
    block::*,
    kura::{BlockStore, LockStatus},
    prelude::*,
    query::store::LiveQueryStore,
    sumeragi::network_topology::Topology,
    wsv::World,
};
use iroha_crypto::KeyPair;
use iroha_data_model::{prelude::*, transaction::TransactionLimits};
use iroha_primitives::unique_vec::UniqueVec;
use tokio::{fs, runtime::Runtime};

async fn measure_block_size_for_n_executors(n_executors: u32) {
    let chain_id = ChainId::from("0");

    let alice_id = AccountId::from_str("alice@test").expect("tested");
    let bob_id = AccountId::from_str("bob@test").expect("tested");
    let xor_id = AssetDefinitionId::from_str("xor#test").expect("tested");
    let alice_xor_id = AssetId::new(xor_id, alice_id);
    let transfer = Transfer::asset_numeric(alice_xor_id, 10u32, bob_id);
    let keypair = KeyPair::random();
    let tx = TransactionBuilder::new(
        chain_id.clone(),
        AccountId::from_str("alice@wonderland").expect("checked"),
    )
    .with_instructions([transfer])
    .sign(&keypair);
    let transaction_limits = TransactionLimits {
        max_instruction_number: 4096,
        max_wasm_size_bytes: 0,
    };
    let tx = AcceptedTransaction::accept(tx, &chain_id, &transaction_limits)
        .expect("Failed to accept Transaction.");
    let dir = tempfile::tempdir().expect("Could not create tempfile.");
    let cfg = Config {
        init_mode: iroha_config::kura::Mode::Strict,
        debug_output_new_blocks: false,
        store_dir: dir.path().to_path_buf(),
    };
    let kura = iroha_core::kura::Kura::new(&cfg).unwrap();
    let _thread_handle = iroha_core::kura::Kura::start(kura.clone());

    let query_handle = LiveQueryStore::test().start();
    let mut wsv = WorldStateView::new(World::new(), kura, query_handle);
    let topology = Topology::new(UniqueVec::new());
    let mut block = BlockBuilder::new(vec![tx], topology, Vec::new())
        .chain(0, &mut wsv)
        .sign(&KeyPair::random());

    for _ in 1..n_executors {
        block = block.sign(&KeyPair::random());
    }
    let mut block_store = BlockStore::new(dir.path(), LockStatus::Unlocked);
    block_store.create_files_if_they_do_not_exist().unwrap();
    block_store.append_block_to_chain(&block.into()).unwrap();

    let metadata = fs::metadata(dir.path().join("blocks.data")).await.unwrap();
    let file_size = Byte::from_bytes(u128::from(metadata.len())).get_appropriate_unit(false);
    println!("For {n_executors} executors: {file_size}");
}

async fn measure_block_size_async() {
    println!("File size of a block with 1 transaction with 1 Transfer instruction is:",);
    for max_faults in 0_u32..5_u32 {
        let n_executors = 3 * max_faults + 1;
        measure_block_size_for_n_executors(n_executors).await;
    }
}

fn measure_block_size(_criterion: &mut Criterion) {
    Runtime::new().unwrap().block_on(measure_block_size_async());
}

criterion_group!(kura, measure_block_size);
criterion_main!(kura);
