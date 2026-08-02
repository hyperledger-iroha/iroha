// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! benches/sha256_vega_mc_zkp.rs
//! Criterion benchmarks for Vega MC {setup, prep_prove, prove, verify}
//! on a batch of SHA-256 single-block compressions.
//!
//! Run with: `RUSTFLAGS="-C target-cpu=native" cargo bench --bench sha256_vega_mc_zkp`
//! Override thread counts with `BENCH_THREADS=1,4,8`.

#[cfg(feature = "jem")]
use tikv_jemallocator::Jemalloc;
#[cfg(feature = "jem")]
#[global_allocator]
static GLOBAL: Jemalloc = tikv_jemallocator::Jemalloc;

use bellpepper::gadgets::{sha256::sha256_compression_function, uint32::UInt32};
use bellpepper_core::{
    ConstraintSystem, SynthesisError,
    boolean::{AllocatedBit, Boolean},
    num::AllocatedNum,
};
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use ff::Field;
use std::{marker::PhantomData, time::Duration};
use vega_prover::{
    provider::T256HyraxEngine,
    traits::{Engine, circuit::VegaCircuit},
    vega_mc_zkp::VegaMcZkSNARK,
};

type E = T256HyraxEngine;

/// Sizes in bytes to benchmark: 1 KB (16 steps) and 2 KB (32 steps).
const SIZES: &[usize] = &[1024, 2048];

/// SHA-256 block size in bytes.
const BLOCK_BYTES: usize = 64;

fn num_steps_for_size(size: usize) -> usize {
    size / BLOCK_BYTES
}

/// Step circuit: proves one SHA-256 compression on a 64-byte block.
///
/// The 512 input bits are allocated as precommitted witnesses; the previous
/// hash state uses the SHA-256 IV (constants).
#[derive(Clone, Debug)]
struct Sha256StepCircuit<Eng: Engine> {
    block: [u8; BLOCK_BYTES],
    _p: PhantomData<Eng>,
}

impl<Eng: Engine> Sha256StepCircuit<Eng> {
    fn new(block: [u8; BLOCK_BYTES]) -> Self {
        Self {
            block,
            _p: PhantomData,
        }
    }
}

impl<Eng: Engine> VegaCircuit<Eng> for Sha256StepCircuit<Eng> {
    fn public_values(&self) -> Result<Vec<Eng::Scalar>, SynthesisError> {
        Ok(vec![Eng::Scalar::ZERO])
    }

    fn shared<CS: ConstraintSystem<Eng::Scalar>>(
        &self,
        _: &mut CS,
    ) -> Result<Vec<AllocatedNum<Eng::Scalar>>, SynthesisError> {
        Ok(vec![])
    }

    fn precommitted<CS: ConstraintSystem<Eng::Scalar>>(
        &self,
        cs: &mut CS,
        _: &[AllocatedNum<Eng::Scalar>],
    ) -> Result<Vec<AllocatedNum<Eng::Scalar>>, SynthesisError> {
        // Allocate 512 bits of block input as witness (big-endian per byte).
        let input_bits: Vec<Boolean> = self
            .block
            .iter()
            .flat_map(|byte| (0..8).rev().map(move |i| (byte >> i) & 1u8 == 1u8))
            .enumerate()
            .map(|(i, b)| {
                AllocatedBit::alloc(cs.namespace(|| format!("block bit {i}")), Some(b))
                    .map(Boolean::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(input_bits.len(), 512);

        // Use SHA-256 IV for the current hash value (constants).
        const IV: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ];
        let current_hash: Vec<UInt32> = IV.iter().map(|&v| UInt32::constant(v)).collect();

        // One SHA-256 compression per step.
        let _next = sha256_compression_function(
            cs.namespace(|| "sha256 compression"),
            &input_bits,
            &current_hash,
        )?;

        let x = AllocatedNum::alloc(cs.namespace(|| "x"), || Ok(Eng::Scalar::ZERO))?;
        x.inputize(cs.namespace(|| "inputize x"))?;

        Ok(vec![])
    }

    fn num_challenges(&self) -> usize {
        0
    }

    fn synthesize<CS: ConstraintSystem<Eng::Scalar>>(
        &self,
        _: &mut CS,
        _: &[AllocatedNum<Eng::Scalar>],
        _: &[AllocatedNum<Eng::Scalar>],
        _: Option<&[Eng::Scalar]>,
    ) -> Result<(), SynthesisError> {
        Ok(())
    }
}

/// Trivial core circuit: just exposes a single public input. Preserves the
/// "N step circuits + 1 core circuit" structure that Vega MC requires.
#[derive(Clone, Debug)]
struct CoreCircuit<Eng: Engine>(PhantomData<Eng>);

impl<Eng: Engine> CoreCircuit<Eng> {
    fn new() -> Self {
        Self(PhantomData)
    }
}

impl<Eng: Engine> VegaCircuit<Eng> for CoreCircuit<Eng> {
    fn public_values(&self) -> Result<Vec<Eng::Scalar>, SynthesisError> {
        Ok(vec![Eng::Scalar::ZERO])
    }

    fn shared<CS: ConstraintSystem<Eng::Scalar>>(
        &self,
        _: &mut CS,
    ) -> Result<Vec<AllocatedNum<Eng::Scalar>>, SynthesisError> {
        Ok(vec![])
    }

    fn precommitted<CS: ConstraintSystem<Eng::Scalar>>(
        &self,
        cs: &mut CS,
        _: &[AllocatedNum<Eng::Scalar>],
    ) -> Result<Vec<AllocatedNum<Eng::Scalar>>, SynthesisError> {
        // One SHA-256 compression, matching the step circuit's shape.
        // Keeps the core circuit under 32,768 constraints (~26,352 for one compression)
        // while still representing a meaningful per-fold "core" workload.
        let input_bits: Vec<Boolean> = (0..512)
            .map(|i| {
                AllocatedBit::alloc(cs.namespace(|| format!("core bit {i}")), Some(false))
                    .map(Boolean::from)
            })
            .collect::<Result<Vec<_>, _>>()?;

        const IV: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ];
        let current_hash: Vec<UInt32> = IV.iter().map(|&v| UInt32::constant(v)).collect();

        let _next = sha256_compression_function(
            cs.namespace(|| "core sha256 compression"),
            &input_bits,
            &current_hash,
        )?;

        let x = AllocatedNum::alloc(cs.namespace(|| "x"), || Ok(Eng::Scalar::ZERO))?;
        x.inputize(cs.namespace(|| "inputize x"))?;
        Ok(vec![])
    }

    fn num_challenges(&self) -> usize {
        0
    }

    fn synthesize<CS: ConstraintSystem<Eng::Scalar>>(
        &self,
        _: &mut CS,
        _: &[AllocatedNum<Eng::Scalar>],
        _: &[AllocatedNum<Eng::Scalar>],
        _: Option<&[Eng::Scalar]>,
    ) -> Result<(), SynthesisError> {
        Ok(())
    }
}

/// Thread counts to benchmark. Override with BENCH_THREADS env var (comma-separated).
/// Defaults are capped at the host's available parallelism to avoid oversubscription.
fn thread_counts() -> Vec<usize> {
    if let Ok(val) = std::env::var("BENCH_THREADS") {
        val.split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect()
    } else {
        let max = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        vec![1, 2, 4, 8, 16]
            .into_iter()
            .filter(|&t| t <= max)
            .collect()
    }
}

fn make_step_circuits(num_steps: usize) -> Vec<Sha256StepCircuit<E>> {
    (0..num_steps)
        .map(|i| Sha256StepCircuit::<E>::new([i as u8; BLOCK_BYTES]))
        .collect()
}

fn vega_mc_zkp_benches(c: &mut Criterion) {
    let thread_counts = thread_counts();

    // Report proof sizes once per size (outside measurements).
    for &size in SIZES {
        let num_steps = num_steps_for_size(size);
        let step_proto = Sha256StepCircuit::<E>::new([0u8; BLOCK_BYTES]);
        let core_proto = CoreCircuit::<E>::new();
        let (pk, _vk) = VegaMcZkSNARK::<E>::setup(&step_proto, &core_proto, num_steps).unwrap();
        let step_circuits = make_step_circuits(num_steps);
        let core_circuit = CoreCircuit::<E>::new();
        let prep =
            VegaMcZkSNARK::<E>::prep_prove(&pk, &step_circuits, &core_circuit, true).unwrap();
        let (proof, _) =
            VegaMcZkSNARK::<E>::prove(&pk, &step_circuits, &core_circuit, prep, true).unwrap();
        let proof_bytes = bincode::serialize(&proof).unwrap();
        println!(
            "Vega MC SHA-256 size={}B num_steps={} (= {} compressions): proof_size={} bytes",
            size,
            num_steps,
            num_steps,
            proof_bytes.len()
        );
    }

    let mut g = c.benchmark_group("vega_mc_zkp_sha256");
    g.sample_size(10);
    g.warm_up_time(Duration::from_millis(100));
    g.measurement_time(Duration::from_secs(10));

    for &size in SIZES {
        let num_steps = num_steps_for_size(size);
        g.throughput(Throughput::Bytes(size as u64));

        for &nthreads in &thread_counts {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(nthreads)
                .build()
                .expect("failed to build rayon pool");

            g.bench_function(format!("setup/{size}/t{nthreads}"), |b| {
                b.iter(|| {
                    pool.install(|| {
                        let step_proto = Sha256StepCircuit::<E>::new([0u8; BLOCK_BYTES]);
                        let core_proto = CoreCircuit::<E>::new();
                        let _ =
                            VegaMcZkSNARK::<E>::setup(&step_proto, &core_proto, num_steps).unwrap();
                    });
                });
            });

            g.bench_function(format!("prep_prove/{size}/t{nthreads}"), |b| {
                b.iter_batched(
                    || {
                        pool.install(|| {
                            let step_proto = Sha256StepCircuit::<E>::new([0u8; BLOCK_BYTES]);
                            let core_proto = CoreCircuit::<E>::new();
                            VegaMcZkSNARK::<E>::setup(&step_proto, &core_proto, num_steps)
                                .unwrap()
                                .0
                        })
                    },
                    |pk| {
                        pool.install(|| {
                            let step_circuits = make_step_circuits(num_steps);
                            let core_circuit = CoreCircuit::<E>::new();
                            let _ = VegaMcZkSNARK::<E>::prep_prove(
                                &pk,
                                &step_circuits,
                                &core_circuit,
                                true,
                            )
                            .unwrap();
                        });
                    },
                    BatchSize::LargeInput,
                );
            });

            g.bench_function(format!("prove/{size}/t{nthreads}"), |b| {
                b.iter_batched(
                    || {
                        pool.install(|| {
                            let step_proto = Sha256StepCircuit::<E>::new([0u8; BLOCK_BYTES]);
                            let core_proto = CoreCircuit::<E>::new();
                            let (pk, _vk) =
                                VegaMcZkSNARK::<E>::setup(&step_proto, &core_proto, num_steps)
                                    .unwrap();
                            let step_circuits = make_step_circuits(num_steps);
                            let core_circuit = CoreCircuit::<E>::new();
                            let prep = VegaMcZkSNARK::<E>::prep_prove(
                                &pk,
                                &step_circuits,
                                &core_circuit,
                                true,
                            )
                            .unwrap();
                            // Warm-up prove
                            let (_proof, prep_back) = VegaMcZkSNARK::<E>::prove(
                                &pk,
                                &step_circuits,
                                &core_circuit,
                                prep,
                                true,
                            )
                            .unwrap();
                            (pk, step_circuits, core_circuit, prep_back)
                        })
                    },
                    |(pk, step_circuits, core_circuit, prep)| {
                        pool.install(|| {
                            let _ = VegaMcZkSNARK::<E>::prove(
                                &pk,
                                &step_circuits,
                                &core_circuit,
                                prep,
                                true,
                            )
                            .unwrap();
                        });
                    },
                    BatchSize::LargeInput,
                );
            });

            g.bench_function(format!("verify/{size}/t{nthreads}"), |b| {
                b.iter_batched(
                    || {
                        pool.install(|| {
                            let step_proto = Sha256StepCircuit::<E>::new([0u8; BLOCK_BYTES]);
                            let core_proto = CoreCircuit::<E>::new();
                            let (pk, vk) =
                                VegaMcZkSNARK::<E>::setup(&step_proto, &core_proto, num_steps)
                                    .unwrap();
                            let step_circuits = make_step_circuits(num_steps);
                            let core_circuit = CoreCircuit::<E>::new();
                            let prep = VegaMcZkSNARK::<E>::prep_prove(
                                &pk,
                                &step_circuits,
                                &core_circuit,
                                true,
                            )
                            .unwrap();
                            let (proof, _) = VegaMcZkSNARK::<E>::prove(
                                &pk,
                                &step_circuits,
                                &core_circuit,
                                prep,
                                true,
                            )
                            .unwrap();
                            (vk, proof)
                        })
                    },
                    |(vk, proof)| {
                        pool.install(|| {
                            proof.verify(&vk, num_steps).unwrap();
                        });
                    },
                    BatchSize::LargeInput,
                );
            });
        }
    }
    g.finish();
}

criterion_group!(benches, vega_mc_zkp_benches);
criterion_main!(benches);
