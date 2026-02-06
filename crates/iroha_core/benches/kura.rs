//! Benchmarks for Kura block size and storage characteristics.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::all)]
#![allow(clippy::disallowed_types)]
use std::fs;

use criterion::Criterion;
use iroha_config::{
    base::WithOrigin,
    parameters::{
        actual::{Kura as Config, LaneConfig},
        defaults::kura::BLOCKS_IN_MEMORY,
    },
};
#[allow(clippy::disallowed_types)]
use iroha_core::{
    block::*,
    kura::BlockStore,
    prelude::*,
    query::store::LiveQueryStore,
    state::{State, World},
    sumeragi::network_topology::Topology,
};
use iroha_crypto::KeyPair;
use iroha_data_model::prelude::*;
use iroha_test_samples::gen_account_in;
use tokio::runtime::Builder;

fn measure_block_size_for_n_executors(n_executors: u32) {
    let dir = tempfile::tempdir().expect("Could not create tempfile.");
    let cfg = Config {
        // Use Fast mode for benches to avoid strict full-scan on empty stores.
        init_mode: iroha_config::kura::InitMode::Fast,
        debug_output_new_blocks: false,
        blocks_in_memory: BLOCKS_IN_MEMORY,
        store_dir: WithOrigin::inline(dir.path().to_path_buf()),
        max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
        merge_ledger_cache_capacity:
            iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: iroha_config::kura::FsyncMode::Batched,
        fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
        block_sync_roster_retention:
            iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
        roster_sidecar_retention:
            iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
    };
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
    let (kura, _) = iroha_core::kura::Kura::new(&cfg, &LaneConfig::default()).unwrap();
    // Use a lightweight, test-friendly handle that doesn't require a running Tokio runtime
    let query_handle = LiveQueryStore::start_test();
    let state = Box::new(State::new(
        World::new(),
        kura,
        query_handle,
        #[cfg(feature = "telemetry")]
        <_>::default(),
    ));

    let (alice_id, alice_keypair) = gen_account_in("test");
    let (bob_id, _bob_keypair) = gen_account_in("test");
    let xor_id = "xor#test".parse().expect("tested");
    let alice_xor_id = AssetId::new(xor_id, alice_id.clone());
    let transfer = Transfer::asset_numeric(
        alice_xor_id,
        iroha_primitives::numeric::Numeric::new(10, 0),
        bob_id,
    );
    let tx = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([transfer])
        .sign(alice_keypair.private_key());
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let crypto_cfg = state.crypto();
    let tx = AcceptedTransaction::accept(
        tx,
        &chain_id,
        max_clock_drift,
        tx_limits,
        crypto_cfg.as_ref(),
    )
    .expect("Failed to accept Transaction.");
    let peer_key_pair = KeyPair::random();
    let peer_id = PeerId::new(peer_key_pair.public_key().clone());
    let topology = Topology::new(vec![peer_id]);
    let mut block: Box<ValidBlock> = {
        let unverified_block = BlockBuilder::new(vec![tx])
            .chain(0, state.view().latest_block().as_deref())
            .sign(peer_key_pair.private_key())
            .unpack(|_| {});

        let mut state_block = Box::new(state.block(unverified_block.header()));
        let block = unverified_block
            .validate_and_record_transactions(state_block.as_mut())
            .unpack(|_| {});
        state_block.commit().unwrap();
        Box::new(block)
    };

    for _ in 1..n_executors {
        block.sign(&peer_key_pair, &topology);
    }
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    block_store
        .append_block_to_chain(block.as_ref().as_ref())
        .unwrap();

    let metadata = fs::metadata(dir.path().join("blocks.data")).unwrap();
    let file_size = metadata.len();
    println!("For {n_executors} executors: {file_size} bytes");
}

fn measure_block_size(_criterion: &mut Criterion) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let rt = Builder::new_multi_thread()
                .enable_all()
                .thread_stack_size(64 * 1024 * 1024)
                .build()
                .unwrap();
            let _guard = rt.enter();
            println!("File size of a block with 1 transaction with 1 Transfer instruction is:",);
            for max_faults in 0_u32..5_u32 {
                let n_executors = 3 * max_faults + 1;
                measure_block_size_for_n_executors(n_executors);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

/// Entry point for the benchmark binary.
fn main() {
    // Silence IVM banner if any path constructs it under the hood during this bench.
    #[allow(unused_imports)]
    {
        use ivm::set_banner_enabled;
        set_banner_enabled(false);
    }
    let mut c = Criterion::default().configure_from_args();
    measure_block_size(&mut c);
    c.final_summary();
}
