//! Benchmark pairing.
//! It measures the pairing of the BN256 curve.
//!
//! To run this benchmark:
//!
//!     cargo bench --bench  pairing

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use ff::Field;
use group::prime::PrimeCurveAffine;
use halo2curves::bn256::Bn256;
use pairing::Engine;
use rand_core::SeedableRng;
use rand_xorshift::XorShiftRng;

const SEED: [u8; 16] = [
    0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06, 0xbc, 0xe5,
];

fn bench_pairing<E: Engine>(c: &mut Criterion, name: &'static str) {
    {
        let mut rng = XorShiftRng::from_seed(SEED);
        let mut group = c.benchmark_group(format!("{} Pairing", name));

        group.significance_level(0.1).sample_size(100);
        group.throughput(Throughput::Elements(1));

        let a = E::Fr::random(&mut rng);
        let b = E::Fr::random(&mut rng);

        let g1 = E::G1Affine::generator();
        let g1_affine = (g1 * a).into();

        let g2 = E::G2Affine::generator();
        let g2_affine = (g2 * b).into();

        group.bench_function(&format!("{} pairing", name), move |b| {
            b.iter(|| E::pairing(&black_box(g1_affine), &black_box(g2_affine)))
        });
        group.finish();
    }
}

fn bench_bn256_pairing(c: &mut Criterion) {
    bench_pairing::<Bn256>(c, "BN256");
}

criterion_group!(benches, bench_bn256_pairing);
criterion_main!(benches);
