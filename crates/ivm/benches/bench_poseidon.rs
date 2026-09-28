//! Benchmarks for Poseidon hash functions used by IVM.
use criterion::{BenchmarkId, Criterion};
use ivm::{poseidon2, poseidon2_many_into, poseidon6, poseidon6_many_into};
use std::hint::black_box;
fn bench_poseidon2(c: &mut Criterion) {
    c.bench_function("poseidon2", |b| {
        b.iter(|| {
            poseidon2(5, 7);
        })
    });
}
fn bench_poseidon6(c: &mut Criterion) {
    let inputs = [1u64, 2, 3, 4, 5, 6];
    c.bench_function("poseidon6", |b| {
        b.iter(|| {
            poseidon6(inputs);
        })
    });
}
fn bench_poseidon2_many(c: &mut Criterion) {
    let mut group = c.benchmark_group("poseidon2_many");
    for &size in &[1usize, 16, 64, 256, 1024] {
        let inputs: Vec<(u64, u64)> = (0..size)
            .map(|idx| (idx as u64, (idx as u64).wrapping_mul(3).wrapping_add(1)))
            .collect();
        let mut result = vec![0; size];
        group.bench_with_input(BenchmarkId::from_parameter(size), &inputs, |b, inputs| {
            b.iter(|| {
                assert!(poseidon2_many_into(black_box(inputs), &mut result));
                black_box(&result);
            });
        });
    }
    group.finish();
}
fn bench_poseidon6_many(c: &mut Criterion) {
    let mut group = c.benchmark_group("poseidon6_many");
    for &size in &[1usize, 16, 64, 256] {
        let inputs: Vec<[u64; 6]> = (0..size)
            .map(|idx| {
                let base = idx as u64;
                [
                    base,
                    base.wrapping_mul(3).wrapping_add(1),
                    base.wrapping_mul(5).wrapping_add(2),
                    base.wrapping_mul(7).wrapping_add(3),
                    base.wrapping_mul(11).wrapping_add(4),
                    base.wrapping_mul(13).wrapping_add(5),
                ]
            })
            .collect();
        let mut result = vec![0; size];
        group.bench_with_input(BenchmarkId::from_parameter(size), &inputs, |b, inputs| {
            b.iter(|| {
                assert!(poseidon6_many_into(black_box(inputs), &mut result));
                black_box(&result);
            });
        });
    }
    group.finish();
}
/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_poseidon2(&mut c);
    bench_poseidon6(&mut c);
    bench_poseidon2_many(&mut c);
    bench_poseidon6_many(&mut c);
    c.final_summary();
}
