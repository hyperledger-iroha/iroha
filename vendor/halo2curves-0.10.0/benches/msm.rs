//! This benchmarks Multi Scalar Multiplication (MSM).
//! It measures `G1` from the BN256 curve.
//!
//! To run this benchmark:
//!
//!     cargo bench --bench msm
//!
//! Caveat:  The multicore benchmark assumes:
//!     1. a multi-core system
//!     2. that the `multicore` feature is enabled.  It is by default.

#[macro_use]
extern crate criterion;

use std::time::SystemTime;

use criterion::{BenchmarkId, Criterion};
use ff::{Field, PrimeField};
use group::prime::PrimeCurveAffine;
use halo2curves::{
    bn256::{Fr as Scalar, G1Affine as Point},
    msm::{msm_best, msm_serial},
};
use rand_core::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;
use rayon::{
    current_thread_index,
    prelude::{IntoParallelIterator, ParallelIterator},
};

const SAMPLE_SIZE: usize = 10;
const SINGLECORE_RANGE: [u8; 6] = [3, 8, 10, 12, 14, 16];
const MULTICORE_RANGE: [u8; 9] = [3, 8, 10, 12, 14, 16, 18, 20, 22];
const SEED: [u8; 16] = [
    0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06, 0xbc, 0xe5,
];

fn generate_curvepoints(k: u8) -> Vec<Point> {
    let n: u64 = {
        assert!(k < 64);
        1 << k
    };

    println!("Generating 2^{k} = {n} curve points..",);
    let timer = SystemTime::now();
    let bases = (0..n)
        .into_par_iter()
        .map_init(
            || {
                let mut thread_seed = SEED;
                let uniq = current_thread_index().unwrap().to_ne_bytes();
                assert!(std::mem::size_of::<usize>() == 8);
                for i in 0..uniq.len() {
                    thread_seed[i] += uniq[i];
                    thread_seed[i + 8] += uniq[i];
                }
                XorShiftRng::from_seed(thread_seed)
            },
            |rng, _| Point::random(rng),
        )
        .collect();
    let end = timer.elapsed().unwrap();
    println!(
        "Generating 2^{k} = {n} curve points took: {} sec.\n\n",
        end.as_secs()
    );
    bases
}

fn generate_coefficients(k: u8, bits: usize) -> Vec<Scalar> {
    let n: u64 = {
        assert!(k < 64);
        1 << k
    };
    let max_val: Option<u128> = match bits {
        1 => Some(1),
        8 => Some(0xff),
        16 => Some(0xffff),
        32 => Some(0xffff_ffff),
        64 => Some(0xffff_ffff_ffff_ffff),
        128 => Some(0xffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff),
        256 => None,
        _ => panic!("unexpected bit size {}", bits),
    };

    println!("Generating 2^{k} = {n} coefficients..",);
    let timer = SystemTime::now();
    let coeffs = (0..n)
        .into_par_iter()
        .map_init(
            || {
                let mut thread_seed = SEED;
                let uniq = current_thread_index().unwrap().to_ne_bytes();
                assert!(std::mem::size_of::<usize>() == 8);
                for i in 0..uniq.len() {
                    thread_seed[i] += uniq[i];
                    thread_seed[i + 8] += uniq[i];
                }
                XorShiftRng::from_seed(thread_seed)
            },
            |rng, _| {
                if let Some(max_val) = max_val {
                    let v_lo = rng.next_u64() as u128;
                    let v_hi = rng.next_u64() as u128;
                    let mut v = v_lo + (v_hi << 64);
                    v &= max_val; // Mask the 128bit value to get a lower number of bits
                    Scalar::from_u128(v)
                } else {
                    Scalar::random(rng)
                }
            },
        )
        .collect();
    let end = timer.elapsed().unwrap();
    println!(
        "Generating 2^{k} = {n} coefficients took: {} sec.\n\n",
        end.as_secs()
    );
    coeffs
}

fn msm(c: &mut Criterion) {
    let mut group = c.benchmark_group("msm");
    let max_k = *SINGLECORE_RANGE
        .iter()
        .chain(MULTICORE_RANGE.iter())
        .max()
        .unwrap_or(&16);
    let bases = generate_curvepoints(max_k);
    let bits = [1, 8, 16, 32, 64, 128, 256];
    let coeffs: Vec<_> = bits
        .iter()
        .map(|b| generate_coefficients(max_k, *b))
        .collect();

    for (b_index, b) in bits.iter().enumerate() {
        for k in SINGLECORE_RANGE {
            let id = format!("{b}b_{k}");
            group
                .bench_function(BenchmarkId::new("singlecore", id), |b| {
                    assert!(k < 64);
                    let n: usize = 1 << k;
                    let mut acc = Point::identity().into();
                    b.iter(|| msm_serial(&coeffs[b_index][..n], &bases[..n], &mut acc));
                })
                .sample_size(10);
        }
        for k in MULTICORE_RANGE {
            let id = format!("{b}b_{k}");
            group
                .bench_function(BenchmarkId::new("multicore", id), |b| {
                    assert!(k < 64);
                    let n: usize = 1 << k;
                    b.iter(|| {
                        msm_best(&coeffs[b_index][..n], &bases[..n]);
                    })
                })
                .sample_size(SAMPLE_SIZE);
        }
    }
    group.finish();
}

criterion_group!(benches, msm);
criterion_main!(benches);
