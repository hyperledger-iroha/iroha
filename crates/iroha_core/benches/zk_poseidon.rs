//! Micro-benchmarks for the chip-backed Poseidon Pow5 path (IPA, Zcash gadgets).
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::all)]
//!
//! Compares repeated 2-input compressor calls implemented via the halo2_gadgets
//! Pow5 chip against a native Pow5 helper inside tiny synthetic circuits. These
//! benches are intended to provide order-of-magnitude signals; absolute numbers
//! depend on the host and are not used for consensus decisions.
//!
//! Run (with features):
//!   cargo bench -p iroha_core --bench zk_poseidon \
//!       --features zk-halo2,zk-halo2-ipa,zk-halo2-ipa-poseidon

#![allow(clippy::needless_range_loop)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2-ipa-poseidon"))]
mod benches {
    use halo2_gadgets::poseidon::{
        Hash as PoseidonHash, Pow5Chip, Pow5Config,
        primitives::{ConstantLength, P128Pow5T3},
    };
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{
            Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
        },
        poly::commitment::Params,
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand::rngs::OsRng;

    use super::*;

    // Local native Pow5 compressor used for comparison
    fn compress2_native(a: Scalar, b: Scalar) -> Scalar {
        let t0 = a + Scalar::from(7u64);
        let t1 = b + Scalar::from(13u64);
        let t0_2 = t0 * t0;
        let t0_4 = t0_2 * t0_2;
        let t0_5 = t0_4 * t0;
        let t1_2 = t1 * t1;
        let t1_4 = t1_2 * t1_2;
        let t1_5 = t1_4 * t1;
        Scalar::from(2) * t0_5 + Scalar::from(3) * t1_5
    }

    #[derive(Clone, Default)]
    struct ChipHarness<const REPS: usize>;
    impl<const REPS: usize> Circuit<Scalar> for ChipHarness<REPS> {
        type Config = (
            Pow5Config<Scalar, 3, 2>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
        );
        type FloorPlanner = SimpleFloorPlanner;
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            // Configure chip and an output column (equality for copy constraints if needed)
            let st0 = meta.advice_column();
            let st1 = meta.advice_column();
            let st2 = meta.advice_column();
            let partial = meta.advice_column();
            let rc_a = meta.fixed_column();
            let rc_b = meta.fixed_column();
            let out = meta.advice_column();
            meta.enable_equality(out);
            let cfg =
                Pow5Chip::<Scalar, 3, 2>::configure(meta, [st0, st1, st2], partial, rc_a, rc_b);
            (cfg, out)
        }
        fn synthesize(
            &self,
            (cfg, out): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            // Run REPS compress2 calls via the gadget; assign outputs to `out`
            let chip = Pow5Chip::<Scalar, 3, 2>::construct(cfg.clone());
            let mut hasher = PoseidonHash::<Scalar, P128Pow5T3, ConstantLength<2>, 3, 2>::init(
                chip,
                layouter.namespace(|| "poseidon_harness"),
            )
            .map_err(|_| PlonkError::Synthesis)?;

            // Fixed inputs to avoid RNG effects; chain the digest
            let mut cur = Scalar::from(1);
            let b = Scalar::from(2);
            for i in 0..REPS {
                let (a_cell, b_cell) = layouter.assign_region(
                    || format!("inputs_{i}"),
                    |mut region| {
                        let a_cell = region.assign_advice(cfg.state[0], 0, Value::known(cur));
                        let b_cell = region.assign_advice(cfg.state[1], 0, Value::known(b));
                        Ok((a_cell, b_cell))
                    },
                )?;
                hasher
                    .update(&[a_cell, b_cell])
                    .map_err(|_| PlonkError::Synthesis)?;
                let digest = hasher.squeeze().map_err(|_| PlonkError::Synthesis)?;
                let d = *digest.value();
                layouter.assign_region(
                    || format!("out_{i}"),
                    |mut region| {
                        region.assign_advice(out, 0, Value::known(d));
                        Ok(())
                    },
                )?;
                cur = d;
            }
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct NativeHarness<const REPS: usize>;
    impl<const REPS: usize> Circuit<Scalar> for NativeHarness<REPS> {
        type Config = halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>;
        type FloorPlanner = SimpleFloorPlanner;
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let out = meta.advice_column();
            out
        }
        fn synthesize(
            &self,
            out: Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let mut cur = Scalar::from(1);
            let b = Scalar::from(2);
            for i in 0..REPS {
                cur = compress2_native(cur, b);
                layouter.assign_region(
                    || format!("out_{i}"),
                    |mut region| {
                        region.assign_advice(out, 0, Value::known(cur));
                        Ok(())
                    },
                )?;
            }
            Ok(())
        }
    }

    fn bench_group<const REPS: usize>(c: &mut Criterion) {
        let mut group = c.benchmark_group(format!("poseidon_pow5_reps_{REPS}"));
        let k = 6u32;
        let params: Params<Curve> = Params::new(k);

        // CHIP path
        let vk_chip: VerifyingKey<Curve> =
            keygen_vk(&params, &ChipHarness::<REPS>::default()).expect("vk");
        let pk_chip =
            keygen_pk(&params, vk_chip.clone(), &ChipHarness::<REPS>::default()).expect("pk");
        group.bench_function(BenchmarkId::new("chip", REPS), |b| {
            b.iter(|| {
                let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
                halo2_proofs::plonk::create_proof::<Curve, Challenge255<Curve>, _, _, _, _>(
                    std::hint::black_box(&params),
                    std::hint::black_box(&pk_chip),
                    std::hint::black_box(&[ChipHarness::<REPS>::default()]),
                    std::hint::black_box(&[&[][..]]),
                    std::hint::black_box(OsRng),
                    std::hint::black_box(&mut transcript),
                )
                .expect("proof");
                std::hint::black_box(transcript.finalize());
            })
        });

        // Native path
        let vk_nat: VerifyingKey<Curve> =
            keygen_vk(&params, &NativeHarness::<REPS>::default()).expect("vk");
        let pk_nat =
            keygen_pk(&params, vk_nat.clone(), &NativeHarness::<REPS>::default()).expect("pk");
        group.bench_function(BenchmarkId::new("native", REPS), |b| {
            b.iter(|| {
                let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
                halo2_proofs::plonk::create_proof::<Curve, Challenge255<Curve>, _, _, _, _>(
                    std::hint::black_box(&params),
                    std::hint::black_box(&pk_nat),
                    std::hint::black_box(&[NativeHarness::<REPS>::default()]),
                    std::hint::black_box(&[&[][..]]),
                    std::hint::black_box(OsRng),
                    std::hint::black_box(&mut transcript),
                )
                .expect("proof");
                std::hint::black_box(transcript.finalize());
            })
        });

        group.finish();
    }

    pub fn criterion_benchmarks(c: &mut Criterion) {
        bench_group::<1>(c);
        bench_group::<8>(c);
        bench_group::<32>(c);
    }
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2-ipa-poseidon"))]
criterion_group!(benches, benches::criterion_benchmarks);
#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2-ipa-poseidon"))]
criterion_main!(benches);
