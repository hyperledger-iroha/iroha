//! Stateful reservation, native constraints and abandonment regressions in both Pasta fields.

use super::*;
use halo2_base::{
    gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    halo2_proofs::{
        circuit::V1,
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
        plonk::Circuit,
    },
};
use snark_verifier::{loader::native::NativeLoader, util::hash::Poseidon};

const TEST_K: usize = 12;
const TEST_ROWS: usize = (1 << TEST_K) - 9;

#[derive(Clone, Debug)]
struct StatefulConfig<F: KagemushaPoseidonFieldV1> {
    base: BaseConfig<F>,
    native: PastaNativePoseidonConfigV1,
}

#[derive(Clone)]
struct StatefulCircuit<F: KagemushaPoseidonFieldV1> {
    base: BaseCircuitBuilder<F>,
    jobs: PastaNativePoseidonJobsV1<F>,
}

impl<F: KagemushaPoseidonFieldV1> Circuit<F> for StatefulCircuit<F> {
    type Config = StatefulConfig<F>;
    type FloorPlanner = V1;
    type Params = BaseCircuitParams;
    fn params(&self) -> Self::Params {
        self.base.config_params.clone()
    }
    fn without_witnesses(&self) -> Self {
        Self {
            base: self.base.deep_clone().unknown(true),
            jobs: self.jobs.clone().unknown(),
        }
    }
    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("stateful native test requires parameters")
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows(TEST_ROWS);
        StatefulConfig {
            base,
            native: PastaNativePoseidonConfigV1::configure::<F>(meta, 2),
        }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        self.base.reset_synthesis_state();
        self.base
            .synthesize(config.base, layouter.namespace(|| "stateful Base"))?;
        self.jobs.synthesize(
            &config.native,
            &mut layouter,
            &self.base.core().copy_manager,
            self.base.witness_gen_only(),
            TEST_ROWS,
        )
    }
}

fn stateful_fixture<F: KagemushaPoseidonFieldV1>(
    witness_gen_only: bool,
    offset: u64,
) -> (StatefulCircuit<F>, Vec<F>) {
    let schedule = [0, 0, 1, 2, 3, 41, 0, 2, 1, 4, 4, 3];
    let mut base = BaseCircuitBuilder::new(witness_gen_only)
        .use_k(TEST_K)
        .use_instance_columns(1);
    let gate = GateChip::default();
    let mut jobs = PastaNativePoseidonJobsV1::new(2, TEST_ROWS).unwrap();
    let mut transcript = jobs.begin_transcript(base.main(0), &schedule).unwrap();
    let mut reference = Poseidon::<F, F, WIDTH, 2>::from_spec(
        &NativeLoader,
        F::kagemusha_poseidon_spec_v1().clone(),
    );
    let mut outputs = Vec::new();
    let mut expected = Vec::new();
    for (segment, count) in schedule.into_iter().enumerate() {
        let values = (0..count)
            .map(|i| F::from(offset + (100 * segment + i) as u64))
            .collect::<Vec<_>>();
        let cells = values
            .iter()
            .map(|value| base.main(0).load_witness(*value))
            .collect::<Vec<_>>();
        transcript.absorb(&cells).unwrap();
        reference.update(&values);
        expected.push(reference.squeeze());
        outputs.push(transcript.squeeze(base.main(0), &gate));
    }
    assert_eq!(
        transcript.finish().unwrap().cell,
        outputs.last().unwrap().cell
    );
    assert_eq!(
        outputs.iter().map(|cell| *cell.value()).collect::<Vec<_>>(),
        expected
    );
    base.assigned_instances = vec![outputs];
    base.calculate_params(Some(9));
    (StatefulCircuit { base, jobs }, expected)
}

#[test]
fn native_stateful_transcript_preserves_all_squeezes_and_bridges_in_both_fields() {
    fn check<F: KagemushaPoseidonFieldV1>() {
        let (circuit, expected) = stateful_fixture::<F>(false, 0);
        MockProver::run(TEST_K as u32, &circuit, vec![expected.clone()])
            .unwrap()
            .assert_satisfied();
        let unknown = circuit.without_witnesses();
        assert_eq!(unknown.jobs.required_rows(), circuit.jobs.required_rows());
        assert_eq!(
            format!("{:?}", unknown.params()),
            format!("{:?}", circuit.params())
        );
        for mutation in 0..3 {
            let mut altered = StatefulCircuit {
                base: circuit.base.deep_clone(),
                jobs: circuit.jobs.clone(),
            };
            let wrong = altered.base.main(0).load_constant(F::from(17));
            match mutation {
                0 => altered.jobs.jobs[0].input[0] = wrong,
                1 => altered.jobs.jobs[1].input[2] = wrong,
                _ => altered.jobs.jobs.last_mut().unwrap().output[1] = wrong,
            }
            altered.base.calculate_params(Some(9));
            assert!(
                MockProver::run(TEST_K as u32, &altered, vec![expected.clone()])
                    .unwrap()
                    .verify()
                    .is_err(),
                "state bridge mutation {mutation}"
            );
        }
        let mut wrong_public = expected;
        wrong_public[1] += F::ONE;
        assert!(
            MockProver::run(TEST_K as u32, &circuit, vec![wrong_public])
                .unwrap()
                .verify()
                .is_err()
        );
    }
    check::<Fp>();
    check::<Fq>();
}

#[test]
fn native_stateful_transcript_failures_cannot_leave_an_accepting_queue_in_both_fields() {
    fn check<F: KagemushaPoseidonFieldV1>() {
        for failure in 0..6 {
            let mut base = BaseCircuitBuilder::<F>::new(false).use_k(TEST_K);
            let gate = GateChip::default();
            let mut jobs = PastaNativePoseidonJobsV1::new(2, TEST_ROWS).unwrap();
            let mut transcript = jobs.begin_transcript(base.main(0), &[1]).unwrap();
            let cell = base.main(0).load_witness(F::from(9));
            match failure {
                0 => {
                    transcript.absorb(&[cell]).unwrap();
                    let _ = transcript.squeeze(base.main(0), &gate);
                    drop(transcript);
                }
                1 => {
                    let _ = transcript.squeeze(base.main(0), &gate);
                    assert!(transcript.finish().is_err());
                }
                2 => {
                    transcript.absorb(&[cell]).unwrap();
                    let _ = transcript.squeeze(base.main(0), &gate);
                    let _ = transcript.squeeze(base.main(0), &gate);
                    assert!(transcript.finish().is_err());
                }
                3 => {
                    assert!(transcript.absorb(&[cell, cell]).is_err());
                    let _ = transcript.squeeze(base.main(0), &gate);
                    assert!(transcript.finish().is_err());
                }
                4 => {
                    transcript.absorb(&[cell]).unwrap();
                    assert!(transcript.finish().is_err());
                }
                _ => {
                    let mut missing = cell;
                    missing.cell = None;
                    assert!(transcript.absorb(&[missing]).is_err());
                    assert!(transcript.finish().is_err());
                }
            }
            assert!(
                jobs.required_rows().is_err(),
                "abandoned/failed reservation {failure}"
            );
            assert!(jobs.clone().unknown().required_rows().is_err());
            assert!(
                jobs.queue_raw(base.main(0), &gate, vec![cell], &[])
                    .is_err()
            );
            base.calculate_params(Some(9));
            assert!(
                MockProver::run(TEST_K as u32, &StatefulCircuit { base, jobs }, vec![]).is_err(),
                "unfinished transcript must fail actual synthesis: {failure}"
            );
        }
        let mut base = BaseCircuitBuilder::<F>::new(false).use_k(TEST_K);
        let mut jobs = PastaNativePoseidonJobsV1::new(2, TEST_ROWS).unwrap();
        let before = base.main(0).advice_len();
        assert!(
            jobs.begin_transcript(base.main(0), &[usize::MAX, usize::MAX, usize::MAX])
                .is_err()
        );
        assert!(jobs.begin_transcript(base.main(0), &[10_000]).is_err());
        assert!(jobs.begin_transcript(base.main(0), &[]).is_err());
        assert_eq!(base.main(0).advice_len(), before);
        assert_eq!(jobs.required_rows(), Ok(0));
    }
    check::<Fp>();
    check::<Fq>();
}

#[test]
fn native_stateful_transcript_real_proofs_reuse_checked_keys_for_changed_witnesses() {
    use ff::Field;
    use halo2_base::halo2_proofs::poly::{VerificationStrategy, commitment::ParamsProver as _};
    use halo2_base::halo2_proofs::{
        SerdeFormat,
        halo2curves::pasta::{EpAffine, EqAffine},
        plonk::{ProvingKey, VerifyingKey, keygen_pk2, keygen_vk_custom},
        poly::ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            multiopen::{ProverIPA, VerifierIPA},
            strategy::SingleStrategy,
        },
        transcript::{
            Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
        },
    };

    macro_rules! check {
        ($curve:ty, $field:ty, $compressed:expr) => {{
            let params = ParamsIPA::<$curve>::new(TEST_K as u32);
            let (circuit, _) = stateful_fixture::<$field>(false, 100);
            let circuit_params = circuit.params();
            let pk =
                keygen_pk2(&params, &circuit, $compressed).expect("native Poseidon key generation");
            let break_points = circuit.base.break_points();
            let pk_bytes = pk.to_bytes(SerdeFormat::Processed);
            let vk_bytes = pk.get_vk().to_bytes(SerdeFormat::Processed);
            let unknown = circuit.without_witnesses();
            let unknown_vk = keygen_vk_custom(&params, &unknown, $compressed)
                .expect("unknown native Poseidon key generation");
            assert_eq!(
                unknown_vk.to_bytes(SerdeFormat::Processed),
                vk_bytes,
                "witness values must not alter fixed schedule or copy mapping"
            );
            let mut input = pk_bytes.as_slice();
            let restored_pk = ProvingKey::<$curve>::read_checked::<_, StatefulCircuit<$field>>(
                &mut input,
                SerdeFormat::Processed,
                TEST_K as u32,
                circuit_params.clone(),
            )
            .expect("checked native Poseidon PK reload");
            assert!(input.is_empty());
            assert_eq!(restored_pk.to_bytes(SerdeFormat::Processed), pk_bytes);
            let mut input = vk_bytes.as_slice();
            let restored_vk = VerifyingKey::<$curve>::read_checked::<_, StatefulCircuit<$field>>(
                &mut input,
                SerdeFormat::Processed,
                TEST_K as u32,
                circuit_params.clone(),
            )
            .expect("checked native Poseidon VK reload");
            assert!(input.is_empty());
            assert_eq!(restored_vk.to_bytes(SerdeFormat::Processed), vk_bytes);
            let cs = restored_vk.cs();
            assert_eq!(cs.degree(), 7);
            assert_eq!(cs.blinding_factors(), 6);
            let protocol = snark_verifier::system::halo2::compile(
                &params,
                &restored_vk,
                snark_verifier::system::halo2::Config::ipa()
                    .with_num_instance(vec![circuit.base.assigned_instances[0].len()]),
            );
            assert_eq!(protocol.quotient.num_chunk(), 6);
            assert_eq!(
                protocol.preprocessed.len(),
                cs.num_fixed_columns() + cs.permutation().get_columns().len()
            );
            assert_eq!(
                protocol.num_witness.iter().sum::<usize>(),
                cs.num_advice_columns()
                    + 3 * cs.lookups().len()
                    + cs.permutation().get_columns().len().div_ceil(5)
                    + 1
            );
            // Build different witnesses using the same PK. The production witness-only path
            // relies on that PK's fixed permutation mapping and retains the complete trace.
            let (mut witness, public) = stateful_fixture::<$field>(true, 200);
            assert_eq!(witness.jobs.required_rows(), circuit.jobs.required_rows());
            assert_eq!(
                witness.base.config_params.num_advice_per_phase,
                circuit_params.num_advice_per_phase
            );
            witness.base.set_params(circuit_params);
            witness.base.set_break_points(break_points);
            let mut transcript = Blake2bWrite::<_, $curve, Challenge255<$curve>>::init(Vec::new());
            let columns: [&[$field]; 1] = [&public];
            halo2_base::halo2_proofs::plonk::create_proof::<
                IPACommitmentScheme<$curve>,
                ProverIPA<'_, $curve>,
                _,
                _,
                _,
                _,
            >(
                &params,
                &restored_pk,
                &[witness],
                &[&columns],
                rand_core_06::OsRng,
                &mut transcript,
            )
            .expect("genuine native Poseidon proof");
            let proof = transcript.finalize();
            let verify = |public: &[$field], proof: &[u8]| {
                let columns: [&[$field]; 1] = [public];
                let mut transcript = Blake2bRead::<_, $curve, Challenge255<$curve>>::init(proof);
                halo2_base::halo2_proofs::plonk::verify_proof::<
                    IPACommitmentScheme<$curve>,
                    VerifierIPA<'_, $curve>,
                    _,
                    _,
                    _,
                >(
                    &params,
                    &restored_vk,
                    SingleStrategy::<$curve>::new(&params),
                    &[&columns],
                    &mut transcript,
                )
            };
            verify(&public, &proof).expect("checked native Poseidon proof verification");
            let mut changed_input = public.clone();
            changed_input[6] += <$field>::ONE;
            assert!(
                verify(&changed_input, &proof).is_err(),
                "changed public challenge must fail"
            );
            let mut changed_output = public.clone();
            *changed_output.last_mut().unwrap() += <$field>::ONE;
            assert!(
                verify(&changed_output, &proof).is_err(),
                "changed digest must fail"
            );
            let mut corrupt = proof.clone();
            corrupt[0] ^= 1;
            assert!(
                verify(&public, &corrupt).is_err(),
                "corrupt proof must fail"
            );
            println!(
                "stateful native Poseidon {} compressed={} PK={} VK={} proof={}",
                stringify!($curve),
                $compressed,
                pk_bytes.len(),
                vk_bytes.len(),
                proof.len()
            );
        }};
    }
    check!(EqAffine, Fp, false);
    check!(EqAffine, Fp, true);
    check!(EpAffine, Fq, false);
    check!(EpAffine, Fq, true);
}
