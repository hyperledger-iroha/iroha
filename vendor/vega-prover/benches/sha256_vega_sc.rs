// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! benches/sha256_vega_sc.rs
//! Criterion benchmarks for Vega SC {setup, prep_prove, prove, verify}
//! on a SHA-256 circuit with varying message lengths.
//!
//! Run with: `RUSTFLAGS="-C target-cpu=native" cargo bench --bench sha256_vega_sc`
#[cfg(feature = "jem")]
use tikv_jemallocator::Jemalloc;
#[cfg(feature = "jem")]
#[global_allocator]
static GLOBAL: Jemalloc = tikv_jemallocator::Jemalloc;

use bellpepper::gadgets::sha256::sha256;
use bellpepper_core::{
    ConstraintSystem, SynthesisError,
    boolean::{AllocatedBit, Boolean},
    num::AllocatedNum,
};
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use ff::{Field, PrimeField, PrimeFieldBits};
use sha2::{Digest, Sha256};
use std::{marker::PhantomData, time::Duration};
use vega_prover::{
    provider::T256HyraxEngine,
    traits::{Engine, circuit::VegaCircuit, snark::R1CSSNARKTrait},
    vega_sc::VegaSNARK,
};

type E = T256HyraxEngine;

#[derive(Clone, Debug)]
struct Sha256Circuit<Scalar: PrimeField> {
    preimage: Vec<u8>,
    _p: PhantomData<Scalar>,
}

impl<Scalar: PrimeField + PrimeFieldBits> Sha256Circuit<Scalar> {
    fn new(preimage: Vec<u8>) -> Self {
        Self {
            preimage,
            _p: PhantomData,
        }
    }
}

impl<E: Engine> VegaCircuit<E> for Sha256Circuit<E::Scalar> {
    fn public_values(&self) -> Result<Vec<<E as Engine>::Scalar>, SynthesisError> {
        let mut hasher = Sha256::new();
        hasher.update(&self.preimage);
        let hash = hasher.finalize();
        let hash_scalars: Vec<<E as Engine>::Scalar> = hash
            .iter()
            .flat_map(|&byte| {
                (0..8).rev().map(move |i| {
                    if (byte >> i) & 1 == 1 {
                        E::Scalar::ONE
                    } else {
                        E::Scalar::ZERO
                    }
                })
            })
            .collect();
        Ok(hash_scalars)
    }

    fn shared<CS: ConstraintSystem<E::Scalar>>(
        &self,
        _: &mut CS,
    ) -> Result<Vec<AllocatedNum<E::Scalar>>, SynthesisError> {
        Ok(vec![])
    }

    fn precommitted<CS: ConstraintSystem<E::Scalar>>(
        &self,
        cs: &mut CS,
        _: &[AllocatedNum<E::Scalar>],
    ) -> Result<Vec<AllocatedNum<E::Scalar>>, SynthesisError> {
        let bit_values: Vec<_> = self
            .preimage
            .clone()
            .into_iter()
            .flat_map(|byte| (0..8).rev().map(move |i| (byte >> i) & 1 == 1))
            .map(Some)
            .collect();
        assert_eq!(bit_values.len(), self.preimage.len() * 8);

        let preimage_bits = bit_values
            .into_iter()
            .enumerate()
            .map(|(i, b)| AllocatedBit::alloc(cs.namespace(|| format!("preimage bit {i}")), b))
            .map(|b| b.map(Boolean::from))
            .collect::<Result<Vec<_>, _>>()?;

        let hash_bits = sha256(cs.namespace(|| "sha256"), &preimage_bits)?;

        let mut hasher = Sha256::new();
        hasher.update(&self.preimage);
        let expected = hasher.finalize();

        let mut expected_bits = expected
            .iter()
            .flat_map(|&byte| (0..8).rev().map(move |i| (byte >> i) & 1 == 1));

        for b in &hash_bits {
            match b {
                Boolean::Is(bit) => {
                    assert_eq!(expected_bits.next().unwrap(), bit.get_value().unwrap())
                }
                Boolean::Not(bit) => {
                    assert_ne!(expected_bits.next().unwrap(), bit.get_value().unwrap())
                }
                Boolean::Constant(_) => unreachable!(),
            }
        }

        for (i, bit) in hash_bits.iter().enumerate() {
            let n = AllocatedNum::alloc_input(cs.namespace(|| format!("public num {i}")), || {
                Ok(
                    if bit.get_value().ok_or(SynthesisError::AssignmentMissing)? {
                        E::Scalar::ONE
                    } else {
                        E::Scalar::ZERO
                    },
                )
            })?;

            cs.enforce(
                || format!("bit == num {i}"),
                |_| bit.lc(CS::one(), E::Scalar::ONE),
                |lc| lc + CS::one(),
                |lc| lc + n.get_variable(),
            );
        }

        Ok(vec![])
    }

    fn num_challenges(&self) -> usize {
        0
    }

    fn synthesize<CS: ConstraintSystem<E::Scalar>>(
        &self,
        _: &mut CS,
        _: &[AllocatedNum<E::Scalar>],
        _: &[AllocatedNum<E::Scalar>],
        _: Option<&[E::Scalar]>,
    ) -> Result<(), SynthesisError> {
        Ok(())
    }
}

/// Thread counts to benchmark. Override with BENCH_THREADS env var (comma-separated).
fn thread_counts() -> Vec<usize> {
    if let Ok(val) = std::env::var("BENCH_THREADS") {
        val.split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect()
    } else {
        vec![1, 4, 8, 16]
    }
}

fn vega_sc_benches(c: &mut Criterion) {
    let sizes = [1024usize, 2048];
    let thread_counts = thread_counts();

    // Report proof sizes once (outside measurements)
    for &size in &sizes {
        let circuit = Sha256Circuit::<<E as Engine>::Scalar>::new(vec![0u8; size]);
        let (pk, _vk) = VegaSNARK::<E>::setup(circuit.clone()).unwrap();
        let prep = VegaSNARK::<E>::prep_prove(&pk, circuit.clone(), true).unwrap();
        let (proof, _) = VegaSNARK::<E>::prove(&pk, circuit, prep, true).unwrap();
        let proof_bytes = bincode::serialize(&proof).unwrap();
        println!(
            "Vega SC SHA-256 msg={}B: proof_size={} bytes",
            size,
            proof_bytes.len()
        );
    }

    let mut g = c.benchmark_group("vega_sc_sha256");
    g.sample_size(10);
    g.warm_up_time(Duration::from_millis(100));
    g.measurement_time(Duration::from_secs(10));

    for &size in &sizes {
        g.throughput(Throughput::Bytes(size as u64));
        for &nthreads in &thread_counts {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(nthreads)
                .build()
                .expect("failed to build rayon pool");

            g.bench_function(format!("setup/{size}/t{nthreads}"), |b| {
                b.iter(|| {
                    pool.install(|| {
                        let circuit = Sha256Circuit::<<E as Engine>::Scalar>::new(vec![0u8; size]);
                        let _ = VegaSNARK::<E>::setup(circuit).unwrap();
                    });
                });
            });

            g.bench_function(format!("prep_prove/{size}/t{nthreads}"), |b| {
                b.iter_batched(
                    || {
                        pool.install(|| {
                            let circuit =
                                Sha256Circuit::<<E as Engine>::Scalar>::new(vec![0u8; size]);
                            VegaSNARK::<E>::setup(circuit).unwrap().0
                        })
                    },
                    |pk| {
                        pool.install(|| {
                            let circuit =
                                Sha256Circuit::<<E as Engine>::Scalar>::new(vec![0u8; size]);
                            let _ = VegaSNARK::<E>::prep_prove(&pk, circuit, true).unwrap();
                        });
                    },
                    BatchSize::LargeInput,
                );
            });

            g.bench_function(format!("prove/{size}/t{nthreads}"), |b| {
                b.iter_batched(
                    || {
                        pool.install(|| {
                            let circuit =
                                Sha256Circuit::<<E as Engine>::Scalar>::new(vec![0u8; size]);
                            let (pk, _vk) = VegaSNARK::<E>::setup(circuit.clone()).unwrap();
                            let prep =
                                VegaSNARK::<E>::prep_prove(&pk, circuit.clone(), true).unwrap();
                            // Warm-up prove
                            let (_proof, prep_back) =
                                VegaSNARK::<E>::prove(&pk, circuit.clone(), prep, true).unwrap();
                            (pk, circuit, prep_back)
                        })
                    },
                    |(pk, circuit, prep)| {
                        pool.install(|| {
                            let _ = VegaSNARK::<E>::prove(&pk, circuit, prep, true).unwrap();
                        });
                    },
                    BatchSize::LargeInput,
                );
            });

            g.bench_function(format!("verify/{size}/t{nthreads}"), |b| {
                b.iter_batched(
                    || {
                        pool.install(|| {
                            let circuit =
                                Sha256Circuit::<<E as Engine>::Scalar>::new(vec![0u8; size]);
                            let (pk, vk) = VegaSNARK::<E>::setup(circuit.clone()).unwrap();
                            let prep =
                                VegaSNARK::<E>::prep_prove(&pk, circuit.clone(), true).unwrap();
                            let (proof, _) =
                                VegaSNARK::<E>::prove(&pk, circuit, prep, true).unwrap();
                            (vk, proof)
                        })
                    },
                    |(vk, proof)| {
                        pool.install(|| {
                            proof.verify(&vk).unwrap();
                        });
                    },
                    BatchSize::LargeInput,
                );
            });
        }
    }
    g.finish();
}

criterion_group!(benches, vega_sc_benches);
criterion_main!(benches);
