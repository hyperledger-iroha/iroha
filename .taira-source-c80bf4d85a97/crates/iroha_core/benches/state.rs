//! Benchmarks for production WSV state commit paths.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[cfg(feature = "bench")]
use core::num::NonZeroU64;
#[cfg(feature = "bench")]
use std::time::Duration;

#[cfg(feature = "bench")]
use criterion::{BatchSize, Criterion};
#[cfg(feature = "bench")]
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};

#[cfg(feature = "bench")]
fn bench_state_write_lock_heavy_world_commit(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_write_lock_heavy_world_commit");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(100));
    for account_count in [128_usize, 2_048] {
        group.bench_function(format!("accounts_{account_count}"), |b| {
            b.iter_batched(
                || {
                    let kura = Kura::blank_kura_for_testing();
                    let query = LiveQueryStore::start_test();
                    State::new_for_testing(World::default(), kura, query)
                },
                |state| {
                    let elapsed = state
                        .commit_heavy_world_accounts_for_bench(
                            NonZeroU64::new(1).expect("non-zero block height"),
                            account_count,
                        )
                        .expect("commit heavy world account benchmark block");
                    assert!(elapsed > Duration::ZERO);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Entry point for the benchmark binary.
#[cfg(feature = "bench")]
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_state_write_lock_heavy_world_commit(&mut c);
    c.final_summary();
}

/// Entry point used when benchmark-only helpers are not compiled.
#[cfg(not(feature = "bench"))]
fn main() {}
