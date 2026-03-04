//! This benchmarks Fast-Fourier Transform (FFT).
//! Since it is over a finite field, it is actually the Number Theoretical
//! Transform (NNT).  It uses the `Fr` scalar field from the BN256 curve.
//!
//! To run this benchmark:
//!
//!     cargo bench -- fft
//!
//! Caveat:  The multicore benchmark assumes:
//!     1. a multi-core system
//!     2. that the `multicore` feature is enabled.  It is by default.

#[macro_use]
extern crate criterion;

use criterion::{BenchmarkId, Criterion};
use group::ff::Field;
use halo2curves_axiom::bn256::Fr as Scalar;
use halo2curves_axiom::fft::best_fft;
use rand_core::OsRng;
use std::ops::Range;
use std::time::SystemTime;

const RANGE: Range<u32> = 3..19;

fn generate_data(k: u32) -> Vec<Scalar> {
    let n = 1 << k;
    let timer = SystemTime::now();
    println!("\n\nGenerating 2^{k} = {n} values..",);
    let data: Vec<Scalar> = (0..n).map(|_| Scalar::random(OsRng)).collect();
    let end = timer.elapsed().unwrap();
    println!(
        "Generating 2^{k} = {n} values took: {} sec.\n\n",
        end.as_secs()
    );
    data
}

fn fft(c: &mut Criterion) {
    let max_k = RANGE.max().unwrap_or(16);
    let mut data = generate_data(max_k);
    let omega = Scalar::random(OsRng);
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
