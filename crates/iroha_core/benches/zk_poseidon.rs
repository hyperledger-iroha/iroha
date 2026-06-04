//! Micro-benchmarks for the local Poseidon Pow5 compatibility path.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::all)]
//!
//! Compares repeated constrained 2-input compressor calls against a native
//! Pow5 helper inside tiny synthetic circuits. These
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
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{
            Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, create_proof, keygen_pk,
            keygen_vk,
        },
        poly::{
            commitment::ParamsProver as _,
            ipa::{
                commitment::{IPACommitmentScheme, ParamsIPA},
                multiopen::ProverIPA,
            },
        },
        transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
    };
    use rand_core_06::OsRng;

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
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; 3],
            halo2_proofs::plonk::Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let state = [
                meta.advice_column(),
                meta.advice_column(),
                meta.advice_column(),
            ];
            let selector = meta.selector();
            meta.create_gate("local_poseidon_pow5", |meta| {
                let s = meta.query_selector(selector);
                let a = meta.query_advice(state[0], halo2_proofs::poly::Rotation::cur());
                let b = meta.query_advice(state[1], halo2_proofs::poly::Rotation::cur());
                let digest = meta.query_advice(state[2], halo2_proofs::poly::Rotation::cur());
                let constant =
                    |value: u64| halo2_proofs::plonk::Expression::Constant(Scalar::from(value));
                let pow5 = |expr: halo2_proofs::plonk::Expression<Scalar>| {
                    let squared = expr.clone() * expr.clone();
                    let fourth = squared.clone() * squared.clone();
                    fourth * expr
                };
                let expected =
                    constant(2) * pow5(a + constant(7)) + constant(3) * pow5(b + constant(13));
                vec![s * (digest - expected)]
            });
            (state, selector)
        }
        fn synthesize(
            &self,
            (state, selector): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            // Fixed inputs to avoid RNG effects; chain the digest.
            let mut cur = Scalar::from(1);
            let b = Scalar::from(2);
            for i in 0..REPS {
                let digest = compress2_native(cur, b);
                layouter.assign_region(
                    || format!("local_poseidon_{i}"),
                    |mut region| {
                        selector.enable(&mut region, 0)?;
                        region.assign_advice(state[0], 0, Value::known(cur));
                        region.assign_advice(state[1], 0, Value::known(b));
                        region.assign_advice(state[2], 0, Value::known(digest));
                        Ok(())
                    },
                )?;
                cur = digest;
            }
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct NativeHarness<const REPS: usize>;
    impl<const REPS: usize> Circuit<Scalar> for NativeHarness<REPS> {
        type Config = halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>;
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

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
        let params: ParamsIPA<Curve> = ParamsIPA::new(k);

        // Constrained local path
        let vk_chip: VerifyingKey<Curve> =
            keygen_vk(&params, &ChipHarness::<REPS>::default()).expect("vk");
        let pk_chip =
            keygen_pk(&params, vk_chip.clone(), &ChipHarness::<REPS>::default()).expect("pk");
        group.bench_function(BenchmarkId::new("local", REPS), |b| {
            b.iter(|| {
                let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
                create_proof::<
                    IPACommitmentScheme<Curve>,
                    ProverIPA<'_, Curve>,
                    Challenge255<Curve>,
                    _,
                    _,
                    _,
                >(
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
                create_proof::<
                    IPACommitmentScheme<Curve>,
                    ProverIPA<'_, Curve>,
                    Challenge255<Curve>,
                    _,
                    _,
                    _,
                >(
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
