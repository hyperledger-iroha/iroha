//! Criterion benchmark driver for applying blocks.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
mod apply_blocks;

use apply_blocks::StateApplyBlocks;
use apply_blocks::common::generate_ids;
use criterion::{BatchSize, Criterion};
use iroha_core::state::World;
use iroha_data_model::{Registrable, account::Account, asset::AssetDefinition, domain::Domain};

fn apply_blocks(c: &mut Criterion) {
    // Ensure instruction registry is initialized for benches using InstructionBox
    iroha_data_model::isi::set_instruction_registry(
        iroha_data_model::instruction_registry::default(),
    );
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building the Runtime");
    let mut group = c.benchmark_group("apply_blocks");
    group.significance_level(0.1).sample_size(10);
    group.bench_function("apply_blocks", |b| {
        b.iter_batched_ref(
            || StateApplyBlocks::setup(rt.handle()),
            |bench| {
                StateApplyBlocks::measure(bench);
            },
            criterion::BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn large_world() -> World {
    let (domain_ids, account_ids, asset_definition_ids) = generate_ids(10, 100, 100);
    let authority = account_ids
        .first()
        .expect("benchmark fixture has accounts")
        .clone();
    let domains = domain_ids
        .into_iter()
        .map(|id| Domain::new(id).build(&authority));
    let accounts = account_ids
        .into_iter()
        .map(|id| Account::new(id).build(&authority));
    let asset_definitions = asset_definition_ids
        .into_iter()
        .map(|id| AssetDefinition::numeric(id).build(&authority));

    World::with(domains, accounts, asset_definitions)
}

fn state_commit(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_commit");
    group.significance_level(0.1).sample_size(10);
    group.bench_function("world_commit_noop_large_world", |b| {
        b.iter_batched(
            large_world,
            |world| {
                let block = world.block();
                block.commit();
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

/// Entry point for the benchmark binary.
fn main() {
    // Silence IVM banner for block-validation benches.
    #[allow(unused_imports)]
    {
        use ivm::set_banner_enabled;
        set_banner_enabled(false);
    }
    let mut c = Criterion::default().configure_from_args();
    apply_blocks(&mut c);
    state_commit(&mut c);
    c.final_summary();
}
