//! Benchmarks for field arithmetic and Poseidon hashing in IVM.
use criterion::{BenchmarkId, Criterion};
#[cfg(target_arch = "aarch64")]
use ivm::field_dispatch::NeonField;
#[cfg(target_arch = "x86_64")]
use ivm::field_dispatch::{Avx2Field, Avx512Field, Sse2Field};
use ivm::{
    bn254_vec::{self as field_vec, FieldElem},
    field_dispatch::{self, FieldArithmetic, ScalarField},
    poseidon2, poseidon6,
};

fn available_backends() -> Vec<(&'static dyn FieldArithmetic, &'static str)> {
    let mut backends: Vec<(&'static dyn FieldArithmetic, &'static str)> =
        vec![(&ScalarField, "scalar")];
    #[cfg(target_arch = "x86_64")]
    {
        backends.push((&Sse2Field, "sse2"));
        if std::is_x86_feature_detected!("avx2") {
            backends.push((&Avx2Field, "avx2"));
        }
        if std::is_x86_feature_detected!("avx512f") {
            backends.push((&Avx512Field, "avx512"));
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            backends.push((&NeonField, "neon"));
        }
    }
    backends
}

fn bench_field_ops(c: &mut Criterion) {
    let a = FieldElem([1, 2, 3, 4]);
    let b = FieldElem([4, 3, 2, 1]);
    let bulk_a: Vec<FieldElem> = (0..256)
        .map(|i| FieldElem::from_u64(i as u64 + 1))
        .collect();
    let bulk_b: Vec<FieldElem> = (0..256)
        .map(|i| FieldElem::from_u64((i * 3 + 7) as u64))
        .collect();

    let backends = available_backends();

    let mut add_group = c.benchmark_group("field_add");
    for (backend, name) in &backends {
        field_dispatch::set_field_impl_for_tests(*backend);
        add_group.bench_function(BenchmarkId::from_parameter(name), |bch| {
            bch.iter(|| {
                std::hint::black_box(field_vec::add(a, b));
            });
        });
    }
    field_dispatch::clear_field_impl_for_tests();
    add_group.finish();

    let mut mul_group = c.benchmark_group("field_mul");
    for (backend, name) in &backends {
        field_dispatch::set_field_impl_for_tests(*backend);
        mul_group.bench_function(BenchmarkId::from_parameter(name), |bch| {
            bch.iter(|| {
                std::hint::black_box(field_vec::mul(a, b));
            });
        });
    }
    field_dispatch::clear_field_impl_for_tests();
    mul_group.finish();

    let mut combo_group = c.benchmark_group("field_combo");
    for (backend, name) in &backends {
        field_dispatch::set_field_impl_for_tests(*backend);
        combo_group.bench_function(BenchmarkId::from_parameter(name), |bch| {
            bch.iter(|| {
                let x = field_vec::add(a, b);
                std::hint::black_box(field_vec::mul(x, a));
            });
        });
    }
    field_dispatch::clear_field_impl_for_tests();
    combo_group.finish();

    let mut bulk_group = c.benchmark_group("field_add_bulk");
    for (backend, name) in &backends {
        field_dispatch::set_field_impl_for_tests(*backend);
        bulk_group.bench_function(BenchmarkId::from_parameter(name), |bch| {
            bch.iter(|| {
                let mut acc = FieldElem([0; 4]);
                for (aa, bb) in bulk_a.iter().zip(&bulk_b) {
                    acc = backend.add(*aa, *bb);
                }
                std::hint::black_box(acc);
            });
        });
    }
    field_dispatch::clear_field_impl_for_tests();
    bulk_group.finish();
}

fn bench_poseidon(c: &mut Criterion) {
    let inputs6 = [1u64, 2, 3, 4, 5, 6];

    let backends = available_backends();

    let mut p2_group = c.benchmark_group("poseidon2");
    for (backend, name) in &backends {
        field_dispatch::set_field_impl_for_tests(*backend);
        p2_group.bench_function(BenchmarkId::from_parameter(name), |bch| {
            bch.iter(|| {
                std::hint::black_box(poseidon2(1, 2));
            });
        });
    }
    field_dispatch::clear_field_impl_for_tests();
    p2_group.finish();

    let mut p6_group = c.benchmark_group("poseidon6");
    for (backend, name) in &backends {
        field_dispatch::set_field_impl_for_tests(*backend);
        p6_group.bench_function(BenchmarkId::from_parameter(name), |bch| {
            bch.iter(|| {
                std::hint::black_box(poseidon6(inputs6));
            });
        });
    }
    field_dispatch::clear_field_impl_for_tests();
    p6_group.finish();
}

/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_field_ops(&mut c);
    bench_poseidon(&mut c);
    c.final_summary();
}
