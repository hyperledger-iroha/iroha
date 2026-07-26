use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use halo2curves_axiom::{bn256::*, ff::Field, ff_ext::Legendre};
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;

pub fn bench_bn256_field(c: &mut Criterion) {
    let mut rng = XorShiftRng::from_seed([
        0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06, 0xbc,
        0xe5,
    ]);

    let a = Fq::random(&mut rng);
    let b = Fq::random(&mut rng);

    #[cfg(not(feature = "asm"))]
    let mut group = c.benchmark_group("BN256 Field Arithmetic (no assembly)");

    #[cfg(feature = "asm")]
    let mut group = c.benchmark_group("BN256 Field Arithmetic (with assembly)");

    group.significance_level(0.1).sample_size(10000);
    group.throughput(Throughput::Elements(1));

    group.bench_function("bn256_fq_add", |bencher| {
        bencher.iter(|| black_box(&a).add(black_box(&b)))
    });
    group.bench_function("bn256_fq_double", |bencher| {
        bencher.iter(|| black_box(&a).double())
    });
    group.bench_function("bn256_fq_sub", |bencher| {
        bencher.iter(|| black_box(&a).sub(black_box(&b)))
    });
    group.bench_function("bn256_fq_neg", |bencher| {
        bencher.iter(|| black_box(&a).neg())
    });
    group.bench_function("bn256_fq_mul", |bencher| {
        bencher.iter(|| black_box(&a).mul(black_box(&b)))
    });
    group.bench_function("bn256_fq_square", |bencher| {
        bencher.iter(|| black_box(&a).square())
    });
    group.bench_function("bn256_fq_invert", |bencher| {
        bencher.iter(|| black_box(&a).invert())
    });
    group.bench_function("bn256_fq_legendre", |bencher| {
        bencher.iter(|| black_box(&a).legendre())
    });
}

criterion_group!(benches, bench_bn256_field);
criterion_main!(benches);
