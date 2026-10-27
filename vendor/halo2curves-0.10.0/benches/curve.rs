//! This benchmarks the basic EC operations.
//! It measures `G1` from the BN256 curve.
//!
//! To run this benchmark:
//!
//!     cargo bench --bench curve

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use ff::Field;
use group::prime::PrimeCurveAffine;
use halo2curves::{bn256::G1, CurveExt};
use rand_core::SeedableRng;
use rand_xorshift::XorShiftRng;

fn bench_curve_ops<G: CurveExt>(c: &mut Criterion, name: &'static str) {
    {
        let mut rng = XorShiftRng::seed_from_u64(3141519u64);

        // Generate 2 random points.
        let mut p1 = G::random(&mut rng);
        let p2 = G::random(&mut rng);
        p1 += p2;

        let p1_affine = G::AffineExt::from(p1);

        let s = G::ScalarExt::random(&mut rng);

        const N: usize = 1000;
        let v: Vec<G> = (0..N).map(|_| p1 + G::random(&mut rng)).collect();

        let mut q = vec![G::AffineExt::identity(); N];

        let mut group = c.benchmark_group(format!("{} arithmetic", name));

        group.significance_level(0.1).sample_size(1000);
        group.throughput(Throughput::Elements(1));

        group.bench_function(&format!("{name} check on curve"), move |b| {
            b.iter(|| black_box(p1).is_on_curve())
        });
        group.bench_function(&format!("{name} check equality"), move |b| {
            b.iter(|| black_box(p1) == black_box(p1))
        });
        group.bench_function(&format!("{name} to affine"), move |b| {
            b.iter(|| G::AffineExt::from(black_box(p1)))
        });
        group.bench_function(&format!("{name} doubling"), move |b| {
            b.iter(|| black_box(p1).double())
        });
        group.bench_function(&format!("{name} addition"), move |b| {
            b.iter(|| black_box(p1).add(&p2))
        });
        group.bench_function(&format!("{name} mixed addition"), move |b| {
            b.iter(|| black_box(p2).add(&p1_affine))
        });
        group.bench_function(&format!("{name} scalar multiplication"), move |b| {
            b.iter(|| black_box(p1) * black_box(s))
        });
        group.bench_function(&format!("{name} batch to affine n={N}"), move |b| {
            b.iter(|| {
                G::batch_normalize(black_box(&v), black_box(&mut q));
            })
        });
        group.finish();
    }
}

fn bench_bn256_ops(c: &mut Criterion) {
    bench_curve_ops::<G1>(c, "BN256")
}

criterion_group!(benches, bench_bn256_ops);
criterion_main!(benches);
