//! This benchmarks the basic hash-to-curve algorithm.
//! It measures `G1` from the BN256 curve.
//!
//! To run this benchmark:
//!
//!     cargo bench --bench hash_to_curve

use core::iter;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use halo2curves::{bn256::G1, CurveExt};
use rand_core::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;

const SEED: [u8; 16] = [
    0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06, 0xbc, 0xe5,
];
fn hash_to_curve<G: CurveExt>(c: &mut Criterion, name: &'static str) {
    {
        let hasher = G::hash_to_curve("test");
        let mut rng = XorShiftRng::from_seed(SEED);
        let message = iter::repeat_with(|| rng.next_u32().to_be_bytes())
            .take(1024)
            .flatten()
            .collect::<Vec<_>>();

        let mut group = c.benchmark_group(format!("{} hash-to-curve", name));

        group.significance_level(0.1).sample_size(100);
        group.throughput(Throughput::Elements(1));

        group.bench_function(&format!("Hash to {name}"), move |b| {
            b.iter(|| hasher(black_box(&message)))
        });
        group.finish();
    }
}

fn hash_to_bn256(c: &mut Criterion) {
    hash_to_curve::<G1>(c, "BN256");
}

criterion_group!(benches, hash_to_bn256);
criterion_main!(benches);
