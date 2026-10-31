//! This benchmarks the basic FF operations.
//! It measures the base field `Fq` and scalar field `Fr` from the BN256 curve.
//!
//! To run this benchmark:
//!
//!     cargo bench --bench field_arith

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use halo2curves::{
    bn256::{Fq, Fr},
    ff::Field,
    ff_ext::Legendre,
};
use rand_core::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;

const SEED: [u8; 16] = [
    0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06, 0xbc, 0xe5,
];
fn bench_field_arithmetic<F: Field + Legendre>(c: &mut Criterion, name: &'static str) {
    let mut rng = XorShiftRng::from_seed(SEED);

    let a = <F as Field>::random(&mut rng);
    let b = <F as Field>::random(&mut rng);
    let exp = rng.next_u64();

    let mut group = c.benchmark_group(format!("{} arithmetic", name));

    group.significance_level(0.1).sample_size(1000);
    group.throughput(Throughput::Elements(1));

    group.bench_function(format!("{}_add", name), |bencher| {
        bencher.iter(|| black_box(&a).add(black_box(&b)))
    });
    group.bench_function(format!("{}_double", name), |bencher| {
        bencher.iter(|| black_box(&a).double())
    });
    group.bench_function(format!("{}_sub", name), |bencher| {
        bencher.iter(|| black_box(&a).sub(black_box(&b)))
    });
    group.bench_function(format!("{}_neg", name), |bencher| {
        bencher.iter(|| black_box(&a).neg())
    });
    group.bench_function(format!("{}_mul", name), |bencher| {
        bencher.iter(|| black_box(&a).mul(black_box(&b)))
    });
    group.bench_function(format!("{}_square", name), |bencher| {
        bencher.iter(|| black_box(&a).square())
    });
    group.bench_function(format!("{}_pow_vartime", name), |bencher| {
        bencher.iter(|| black_box(&a).pow_vartime(black_box(&[exp])))
    });
    group.bench_function(format!("{}_invert", name), |bencher| {
        bencher.iter(|| black_box(&a).invert())
    });
    group.bench_function(format!("{}_legendre", name), |bencher| {
        bencher.iter(|| black_box(&a).legendre())
    });
    group.finish()
}

fn bench_bn256_base_field(c: &mut Criterion) {
    bench_field_arithmetic::<Fq>(c, "Fq")
}
fn bench_bn256_scalar_field(c: &mut Criterion) {
    bench_field_arithmetic::<Fr>(c, "Fr")
}

criterion_group!(benches, bench_bn256_base_field, bench_bn256_scalar_field);
criterion_main!(benches);
