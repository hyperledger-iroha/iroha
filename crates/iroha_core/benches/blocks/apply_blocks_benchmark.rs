//! Criterion benchmark driver for applying blocks.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
mod apply_blocks;

use apply_blocks::StateApplyBlocks;
use criterion::Criterion;

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
    c.final_summary();
}
