//! This benchmarks Fast-Fourier Transform (FFT).
//! Since it is over a finite field, it is actually the Number Theoretical
//! Transform (NNT).  It uses the `Fr` scalar field from the BN256 curve.
//!
//! To run this benchmark:
//!
//!     cargo bench --bench fft
//!
//! Caveat:  The multicore benchmark assumes:
//!     1. a multi-core system
//!     2. that the `multicore` feature is enabled.  It is by default.

#[macro_use]
extern crate criterion;

use std::{ops::Range, time::SystemTime};

use criterion::{BenchmarkId, Criterion};
use group::ff::Field;
use halo2curves::{bn256::Fr as Scalar, fft::best_fft};
use rand_core::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;

const RANGE: Range<u32> = 3..19;
const SEED: [u8; 16] = [
    0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06, 0xbc, 0xe5,
];

fn generate_data(k: u32, mut rng: impl RngCore) -> Vec<Scalar> {
    let n = 1 << k;
    let timer = SystemTime::now();
    println!("\n\nGenerating 2^{k} = {n} values..",);
    let data: Vec<Scalar> = (0..n).map(|_| Scalar::random(&mut rng)).collect();
    let end = timer.elapsed().unwrap();
    println!(
        "Generating 2^{k} = {n} values took: {} sec.\n\n",
        end.as_secs()
    );
    data
}

fn fft(c: &mut Criterion) {
    let max_k = RANGE.max().unwrap_or(16);
    let mut rng = XorShiftRng::from_seed(SEED);
    let mut data = generate_data(max_k, &mut rng);
    let omega = Scalar::random(&mut rng);
    let mut group = c.benchmark_group("fft");
    for k in RANGE {
        group.bench_function(BenchmarkId::new("k", k), |b| {
            let n = 1 << k;
            assert!(n <= data.len());
            b.iter(|| {
                best_fft(&mut data[..n], omega, k);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, fft);
criterion_main!(benches);
