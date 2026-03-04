//! Micro-benchmark for native IPA open verification to aid gas calibration.
//!
//! Runs a small suite across a few vector sizes and reports ns/op. Use these
//! results to adjust `ivm::gas::cost_of_zk_ipa_open` coefficients.

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use iroha_zkp_halo2::{Params, Polynomial, PrimeField64, Transcript};

fn bench_verify_open(c: &mut Criterion) {
    let sizes = [8usize, 16, 32, 64];
    for &n in &sizes {
        c.bench_function(&format!("ipa_verify_open_n{n}"), |b| {
            b.iter_batched(
                || {
                    let params = Params::new(n).unwrap();
                    let coeffs: Vec<_> = (0..n as u64).map(PrimeField64::from).collect();
                    let poly = Polynomial::from_coeffs(coeffs);
                    let p_g = poly.commit(&params).unwrap();
                    let z = PrimeField64::from(7u64);
                    let mut tr = Transcript::new("CALIB");
                    let (proof, t) = poly.open(&params, &mut tr, z, p_g).unwrap();
                    (params, z, p_g, t, proof)
                },
                |(params, z, p_g, t, proof)| {
                    let mut tr_v = Transcript::new("CALIB");
                    Polynomial::verify_open(&params, &mut tr_v, z, p_g, t, &proof).unwrap();
                },
                BatchSize::SmallInput,
            )
        });
    }
}

criterion_group!(benches, bench_verify_open);
criterion_main!(benches);
