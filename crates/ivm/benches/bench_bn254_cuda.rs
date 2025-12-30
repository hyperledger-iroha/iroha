//! Benchmarks for BN254 field operations comparing CPU backends against the CUDA kernels.
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ivm::{
    bn254_vec::{self as field_vec, FieldElem},
    field_dispatch::{self, FieldArithmetic, ScalarField},
};

#[cfg(feature = "cuda")]
fn has_cuda_backend() -> bool {
    ivm::cuda_available()
}

#[cfg(not(feature = "cuda"))]
fn has_cuda_backend() -> bool {
    false
}

fn bench_add(c: &mut Criterion) {
    let a = FieldElem([1, 2, 3, 4]);
    let b = FieldElem([9, 8, 7, 6]);
    let mut group = c.benchmark_group("bn254_add");

    let scalar_backend: &'static dyn FieldArithmetic = &ScalarField;
    field_dispatch::set_field_impl_for_tests(scalar_backend);
    group.bench_function(BenchmarkId::new("cpu", "scalar"), |bch| {
        bch.iter(|| black_box(field_vec::add(a, b)));
    });
    field_dispatch::clear_field_impl_for_tests();

    if has_cuda_backend() {
        // Prime the CUDA backend once to avoid measuring lazy init.
        let _ = ivm::bn254_add_cuda(a.0, b.0);
        group.bench_function(BenchmarkId::new("cuda", "gpu0"), |bch| {
            bch.iter(|| {
                black_box(ivm::bn254_add_cuda(a.0, b.0).unwrap_or_else(|| field_vec::add(a, b).0))
            });
        });
    } else {
        eprintln!("bn254_add: CUDA backend disabled, skipping GPU benchmark");
    }

    group.finish();
}

fn bench_mul(c: &mut Criterion) {
    let a = FieldElem([1, 2, 3, 4]);
    let b = FieldElem([4, 3, 2, 1]);
    let mut group = c.benchmark_group("bn254_mul");

    let scalar_backend: &'static dyn FieldArithmetic = &ScalarField;
    field_dispatch::set_field_impl_for_tests(scalar_backend);
    group.bench_function(BenchmarkId::new("cpu", "scalar"), |bch| {
        bch.iter(|| black_box(field_vec::mul(a, b)));
    });
    field_dispatch::clear_field_impl_for_tests();

    if has_cuda_backend() {
        let _ = ivm::bn254_mul_cuda(a.0, b.0);
        group.bench_function(BenchmarkId::new("cuda", "gpu0"), |bch| {
            bch.iter(|| {
                black_box(ivm::bn254_mul_cuda(a.0, b.0).unwrap_or_else(|| field_vec::mul(a, b).0))
            });
        });
    } else {
        eprintln!("bn254_mul: CUDA backend disabled, skipping GPU benchmark");
    }

    group.finish();
}

fn bench_bn254_cuda(c: &mut Criterion) {
    bench_add(c);
    bench_mul(c);
}

criterion_group!(benches, bench_bn254_cuda);
criterion_main!(benches);
