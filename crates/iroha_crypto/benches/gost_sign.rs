//! Benchmarks for signature generation and verification across TC26 GOST and baseline curves.

use std::hint::black_box;
#[cfg(feature = "gost")]
use std::time::Duration;

#[cfg(feature = "gost")]
use criterion::{BenchmarkId, Criterion};
#[cfg(feature = "gost")]
use iroha_crypto::{Algorithm, KeyPair, Signature};

/// Seed + metadata for a single benchmark target.
#[cfg(feature = "gost")]
struct BenchTarget {
    name: &'static str,
    algorithm: Algorithm,
    seed: &'static [u8],
    message: &'static [u8],
}

#[cfg(feature = "gost")]
const TARGETS: &[BenchTarget] = &[
    BenchTarget {
        name: "ed25519",
        algorithm: Algorithm::Ed25519,
        seed: b"iroha-bench-ed25519-seed",
        message: b"benchmark payload for ed25519",
    },
    BenchTarget {
        name: "secp256k1",
        algorithm: Algorithm::Secp256k1,
        seed: b"iroha-bench-secp256k1-seed",
        message: b"benchmark payload for secp256k1",
    },
    BenchTarget {
        name: "gost256_paramset_a",
        algorithm: Algorithm::Gost3410_2012_256ParamSetA,
        seed: b"iroha-gost-256-a",
        message: b"benchmark payload for gost256a",
    },
    BenchTarget {
        name: "gost256_paramset_b",
        algorithm: Algorithm::Gost3410_2012_256ParamSetB,
        seed: b"iroha-gost-256-b",
        message: b"benchmark payload for gost256b",
    },
    BenchTarget {
        name: "gost256_paramset_c",
        algorithm: Algorithm::Gost3410_2012_256ParamSetC,
        seed: b"iroha-gost-256-c",
        message: b"benchmark payload for gost256c",
    },
    BenchTarget {
        name: "gost512_paramset_a",
        algorithm: Algorithm::Gost3410_2012_512ParamSetA,
        seed: b"iroha-gost-512-a",
        message: b"benchmark payload for gost512a",
    },
    BenchTarget {
        name: "gost512_paramset_b",
        algorithm: Algorithm::Gost3410_2012_512ParamSetB,
        seed: b"iroha-gost-512-b",
        message: b"benchmark payload for gost512b",
    },
];

/// Benchmark signing and verification throughput across selected algorithms.
#[cfg(feature = "gost")]
fn bench_sign_verify(c: &mut Criterion) {
    let mut group = c.benchmark_group("sign_verify");
    group.sample_size(60);
    group.measurement_time(Duration::from_secs(6));

    for target in TARGETS {
        let key_pair = KeyPair::from_seed(target.seed.to_vec(), target.algorithm);
        let public = key_pair.public_key().clone();
        let private = key_pair.private_key().clone();

        group.bench_with_input(
            BenchmarkId::new("sign_verify", target.name),
            target,
            move |b, target| {
                let public = public.clone();
                let private = private.clone();
                b.iter(|| {
                    let message = black_box(target.message);
                    let signature = Signature::new(&private, message);
                    signature
                        .verify(&public, message)
                        .expect("signature must verify");
                });
            },
        );
    }

    group.finish();
}

/// Criterion harness entry point used when `gost` is enabled.
#[cfg(feature = "gost")]
fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    bench_sign_verify(&mut criterion);
    criterion.final_summary();
}

#[cfg(not(feature = "gost"))]
fn main() {}
