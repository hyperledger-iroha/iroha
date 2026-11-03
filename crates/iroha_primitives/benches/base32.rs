//! Throughput of lowercase base32 encoding, including output allocation.
//!
//! Run with `cargo iroha-fast -- bench -p iroha_primitives --bench base32`.
//! The short inputs cover hash identifiers and DNS labels; larger inputs track
//! throughput without adding an external encoder to the dependency graph.
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};
use iroha_primitives::base32::encode_lower;

/// Measure full encoding and output disposal at representative input lengths.
fn encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("base32_lower");
    for length in [20_usize, 32, 64, 256, 8192] {
        let input: Vec<_> = (0..length).map(|i| (i * 173 + 41) as u8).collect();
        group.throughput(Throughput::Bytes(length as u64));
        group.bench_with_input(BenchmarkId::from_parameter(length), &input, |b, input| {
            b.iter(|| black_box(encode_lower(black_box(input))));
        });
    }
    group.finish();
}

/// Criterion entry point for base32 measurements.
fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    encode(&mut criterion);
    criterion.final_summary();
}
