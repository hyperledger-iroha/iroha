//! Benchmarks for BN254 field operations comparing CPU backends against the CUDA kernels.
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ivm::{
    bn254_vec::{self as field_vec, FieldElem},
    field_dispatch::{self, FieldArithmetic, ScalarField},
};
use std::hint::black_box;
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
    if has_cuda_backend() && ivm::bn254_add_cuda(a.0, b.0).is_some() {
        group.bench_function(BenchmarkId::new("cuda", "gpu0"), |bch| {
            bch.iter(|| {
                black_box(
                    ivm::bn254_add_cuda(a.0, b.0)
                        .expect("CUDA benchmark must execute its named backend"),
                )
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
    if has_cuda_backend() && ivm::bn254_mul_cuda(a.0, b.0).is_some() {
        group.bench_function(BenchmarkId::new("cuda", "gpu0"), |bch| {
            bch.iter(|| {
                black_box(
                    ivm::bn254_mul_cuda(a.0, b.0)
                        .expect("CUDA benchmark must execute its named backend"),
                )
            });
        });
    } else {
        eprintln!("bn254_mul: CUDA backend disabled, skipping GPU benchmark");
    }
    group.finish();
}
fn bench_add_batch(c: &mut Criterion) {
    let lhs: Vec<[u64; 4]> = (0..1024)
        .map(|idx| FieldElem::from_u64(idx as u64 + 1).0)
        .collect();
    let rhs: Vec<[u64; 4]> = (0..1024)
        .map(|idx| FieldElem::from_u64((idx as u64).wrapping_mul(3) + 7).0)
        .collect();
    let mut group = c.benchmark_group("bn254_add_batch");
    let mut output = vec![[0; 4]; lhs.len()];
    let scalar_backend: &'static dyn FieldArithmetic = &ScalarField;
    field_dispatch::set_field_impl_for_tests(scalar_backend);
    group.bench_function(BenchmarkId::new("cpu", "scalar_1024"), |bch| {
        bch.iter(|| {
            for ((out, a), b) in output.iter_mut().zip(&lhs).zip(&rhs) {
                *out = field_vec::add_scalar(FieldElem(*a), FieldElem(*b)).0;
            }
            black_box(&output);
        });
    });
    field_dispatch::clear_field_impl_for_tests();
    if has_cuda_backend() && ivm::bn254_add_batch_cuda_into(&lhs, &rhs, &mut output) {
        group.bench_function(BenchmarkId::new("cuda", "gpu_1024"), |bch| {
            bch.iter(|| {
                assert!(
                    ivm::bn254_add_batch_cuda_into(&lhs, &rhs, &mut output),
                    "CUDA benchmark must execute its named backend"
                );
                black_box(&output);
            });
        });
    } else {
        eprintln!("bn254_add_batch: qualified CUDA kernel unavailable, skipping GPU benchmark");
    }
    group.finish();
}
fn bench_mul_batch(c: &mut Criterion) {
    let lhs: Vec<[u64; 4]> = (0..1024)
        .map(|idx| FieldElem::from_u64(idx as u64 + 11).0)
        .collect();
    let rhs: Vec<[u64; 4]> = (0..1024)
        .map(|idx| FieldElem::from_u64((idx as u64).wrapping_mul(5) + 13).0)
        .collect();
    let mut group = c.benchmark_group("bn254_mul_batch");
    let mut output = vec![[0; 4]; lhs.len()];
    let scalar_backend: &'static dyn FieldArithmetic = &ScalarField;
    field_dispatch::set_field_impl_for_tests(scalar_backend);
    group.bench_function(BenchmarkId::new("cpu", "scalar_1024"), |bch| {
        bch.iter(|| {
            for ((out, a), b) in output.iter_mut().zip(&lhs).zip(&rhs) {
                *out = field_vec::mul_scalar(FieldElem(*a), FieldElem(*b)).0;
            }
            black_box(&output);
        });
    });
    field_dispatch::clear_field_impl_for_tests();
    if has_cuda_backend() && ivm::bn254_mul_batch_cuda_into(&lhs, &rhs, &mut output) {
        group.bench_function(BenchmarkId::new("cuda", "gpu_1024"), |bch| {
            bch.iter(|| {
                assert!(
                    ivm::bn254_mul_batch_cuda_into(&lhs, &rhs, &mut output),
                    "CUDA benchmark must execute its named backend"
                );
                black_box(&output);
            });
        });
    } else {
        eprintln!("bn254_mul_batch: qualified CUDA kernel unavailable, skipping GPU benchmark");
    }
    group.finish();
}
fn bench_bn254_cuda(c: &mut Criterion) {
    bench_add(c);
    bench_mul(c);
    bench_add_batch(c);
    bench_mul_batch(c);
}
criterion_group!(benches, bench_bn254_cuda);
criterion_main!(benches);
