use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ff::Field;
use group::prime::PrimeCurveAffine;
use halo2curves_axiom::secp256k1::Secp256k1;
use pasta_curves::arithmetic::CurveExt;
use rand_core::OsRng;

fn criterion_benchmark<G: CurveExt>(c: &mut Criterion) {
    // G1Projective
    {
        let name = "GProjective";
        let p1 = G::random(OsRng);
        let p2 = G::random(OsRng);
        let p1_affine = G::AffineExt::from(p1);
        let s = G::ScalarExt::random(OsRng);

        const N: usize = 1000;
        let v = vec![G::generator(); N];
        let mut q = vec![G::AffineExt::identity(); N];

        c.bench_function(&format!("{name} check on curve"), move |b| {
            b.iter(|| black_box(p1).is_on_curve())
        });
        c.bench_function(&format!("{name} check equality"), move |b| {
            b.iter(|| black_box(p1) == black_box(p1))
        });
        c.bench_function(&format!("{name} to affine"), move |b| {
            b.iter(|| G::AffineExt::from(black_box(p1)))
        });
        c.bench_function(&format!("{name} doubling"), move |b| {
            b.iter(|| black_box(p1).double())
        });
        c.bench_function(&format!("{name} addition"), move |b| {
            b.iter(|| black_box(p1).add(&p2))
        });
        c.bench_function(&format!("{name} mixed addition"), move |b| {
            b.iter(|| black_box(p2).add(&p1_affine))
        });
        c.bench_function(&format!("{name} scalar multiplication"), move |b| {
            b.iter(|| black_box(p1) * black_box(s))
        });
        c.bench_function(&format!("{name} batch to affine n={N}"), move |b| {
            b.iter(|| {
                G::batch_normalize(black_box(&v), black_box(&mut q));
                black_box(&q)[0]
            })
        });
    }
}

criterion_group!(benches, criterion_benchmark<Secp256k1>);
criterion_main!(benches);
