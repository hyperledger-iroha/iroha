//! Benchmarks for Kura block size and storage characteristics.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::all)]
#![allow(clippy::disallowed_types)]
use std::fs;
#[cfg(feature = "bench")]
use std::{num::NonZeroUsize, sync::Arc, time::Duration};

#[cfg(feature = "bench")]
use criterion::BatchSize;
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

#[cfg(feature = "bench")]
struct BenchBlocks {
    leader_key: KeyPair,
    prev_block: Option<Arc<SignedBlock>>,
}

#[cfg(feature = "bench")]
impl BenchBlocks {
    fn new() -> Self {
        Self {
            leader_key: KeyPair::random(),
            prev_block: None,
        }
    }

    fn next(&mut self) -> Arc<SignedBlock> {
        let latest = self.prev_block.as_deref();
        let block: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
            .chain(0, latest)
            .sign(self.leader_key.private_key())
            .unpack(|_| {})
            .into();
        let block = Arc::new(block);
        self.prev_block = Some(Arc::clone(&block));
        block
    }
}

#[cfg(feature = "bench")]
fn budget_bench_config(dir: &tempfile::TempDir) -> Config {
    kura_bench_config(dir, BLOCKS_IN_MEMORY)
}

#[cfg(feature = "bench")]
fn eviction_bench_config(dir: &tempfile::TempDir) -> Config {
    kura_bench_config(dir, NonZeroUsize::new(1).expect("non-zero retained tail"))
}

#[cfg(feature = "bench")]
fn kura_bench_config(dir: &tempfile::TempDir, blocks_in_memory: NonZeroUsize) -> Config {
    Config {
        init_mode: iroha_config::kura::InitMode::Fast,
        debug_output_new_blocks: false,
        blocks_in_memory,
        store_dir: WithOrigin::inline(dir.path().to_path_buf()),
        max_disk_usage_bytes: iroha_config::base::util::Bytes(u64::MAX / 4),
        merge_ledger_cache_capacity:
            iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: iroha_config::kura::FsyncMode::Batched,
        fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
        block_sync_roster_retention:
            iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
        roster_sidecar_retention:
            iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
        eviction_required_replicas:
            iroha_config::parameters::defaults::kura::EVICTION_REQUIRED_REPLICAS,
    }
}

#[cfg(feature = "bench")]
fn seeded_eviction_bench(
    evictable_history: usize,
) -> (tempfile::TempDir, Arc<iroha_core::kura::Kura>, u64) {
    let dir = tempfile::tempdir().expect("create Kura eviction benchmark directory");
    let cfg = eviction_bench_config(&dir);
    let retained_tail = cfg.blocks_in_memory.get();
    let (kura, _) = iroha_core::kura::Kura::new(&cfg, &LaneConfig::default())
        .expect("initialize Kura for eviction benchmark");
    let mut blocks = BenchBlocks::new();
    let persisted_blocks = 1_usize
        .saturating_add(evictable_history)
        .saturating_add(retained_tail);
    for _ in 0..persisted_blocks {
        let block = blocks.next();
        kura.persist_block_immediate_for_bench(&block)
            .expect("persist eviction benchmark block");
    }

    let mut bytes_needed = 0_u64;
    for height in 2..=evictable_history.saturating_add(1) {
        let height = NonZeroUsize::new(height).expect("non-zero height");
        let payload_len = kura
            .advertise_required_replicas_for_bench(height)
            .expect("advertise required replicas for eviction benchmark");
        bytes_needed = bytes_needed.saturating_add(payload_len);
    }
    (dir, kura, bytes_needed)
}

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
        eviction_required_replicas:
            iroha_config::parameters::defaults::kura::EVICTION_REQUIRED_REPLICAS,
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
    let xor_id = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("test", "universal").unwrap(),
        "xor".parse().unwrap(),
    );
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

#[cfg(feature = "bench")]
fn bench_storage_budget_cached_pending_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("kura_storage_budget_cached_pending_depth");
    for pending_depth in [0_usize, 128, 2_048] {
        group.bench_function(format!("pending_depth_{pending_depth}"), |b| {
            let dir = tempfile::tempdir().expect("create Kura benchmark directory");
            let cfg = budget_bench_config(&dir);
            let (kura, _) = iroha_core::kura::Kura::new(&cfg, &LaneConfig::default())
                .expect("initialize Kura for budget benchmark");
            let mut blocks = BenchBlocks::new();
            for _ in 0..pending_depth {
                kura.append_pending_block_for_bench(blocks.next());
            }
            let candidate = blocks.next();
            kura.check_storage_budget_for_bench(candidate.as_ref())
                .expect("warm pending budget cache");
            b.iter(|| {
                kura.check_storage_budget_for_bench(candidate.as_ref())
                    .expect("cached pending budget check should fit")
            });
        });
    }
    group.finish();
}

#[cfg(feature = "bench")]
fn bench_eviction_long_history_compaction(c: &mut Criterion) {
    let mut group = c.benchmark_group("kura_eviction_long_history_compaction");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(100));
    for evictable_history in [64_usize, 512] {
        group.bench_function(format!("evictable_blocks_{evictable_history}"), |b| {
            b.iter_batched(
                || seeded_eviction_bench(evictable_history),
                |(_dir, kura, bytes_needed)| {
                    let freed = kura
                        .evict_block_bodies_for_bench(bytes_needed)
                        .expect("evict benchmark block bodies");
                    assert_eq!(freed, bytes_needed);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
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
    #[cfg(feature = "bench")]
    bench_storage_budget_cached_pending_depth(&mut c);
    #[cfg(feature = "bench")]
    bench_eviction_long_history_compaction(&mut c);
    c.final_summary();
}
