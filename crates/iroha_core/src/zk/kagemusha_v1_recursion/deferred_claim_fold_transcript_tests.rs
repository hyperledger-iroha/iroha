//! Both-field comparisons using the actual dependency fold reader and deferred verifier.
//!
//! The deterministic transcript bytes below are shape fixtures, not valid monetary proofs.
//! Tests compare all challenges, canonical bytes and every deferred equation; reciprocal
//! equation enforcement and genuine complete Claim proofs remain separate qualification gates.

use super::*;
use crate::zk::pasta_native_poseidon::PastaNativePoseidonConfigV1;
use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig};
use halo2_proofs::{
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::{
        group::Curve as _,
        pasta::{EpAffine, EqAffine},
    },
    plonk::{Circuit, ConstraintSystem, Error as Halo2Error},
};
use snark_verifier::util::arithmetic::{Domain, root_of_unity};

const TEST_K: usize = 12;
const TEST_ROWS: usize = (1 << TEST_K) - 9;

#[derive(Clone, Debug)]
struct FoldTestConfig<F: KagemushaPoseidonFieldV1> {
    base: BaseConfig<F>,
    native: PastaNativePoseidonConfigV1,
}

#[derive(Clone)]
struct FoldTestCircuit<F: KagemushaPoseidonFieldV1> {
    base: BaseCircuitBuilder<F>,
    jobs: PastaNativePoseidonJobsV1<F>,
}

impl<F: KagemushaPoseidonFieldV1> Circuit<F> for FoldTestCircuit<F> {
    type Config = FoldTestConfig<F>;
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
        unreachable!("Claim fold transcript test uses parameters")
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows(TEST_ROWS);
        FoldTestConfig {
            base,
            native: PastaNativePoseidonConfigV1::configure::<F>(meta, 2),
        }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Halo2Error> {
        self.base.reset_synthesis_state();
        self.base
            .synthesize(config.base, layouter.namespace(|| "fold transcript Base"))?;
        self.jobs.synthesize(
            &config.native,
            &mut layouter,
            &self.base.core().copy_manager,
            self.base.witness_gen_only(),
            TEST_ROWS,
        )
    }
}

struct RecordingTranscript<'chip, C, T>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    inner: T,
    challenges: Vec<AssignedValue<C::ScalarExt>>,
    marker: std::marker::PhantomData<&'chip C>,
}

impl<C, T> RecordingTranscript<'_, C, T>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    fn new(inner: T) -> Self {
        Self {
            inner,
            challenges: Vec::new(),
            marker: std::marker::PhantomData,
        }
    }
}

impl<'chip, C, T> Transcript<C, DeferredLoader<'chip, C>> for RecordingTranscript<'chip, C, T>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
    T: Transcript<C, DeferredLoader<'chip, C>>,
{
    fn loader(&self) -> &DeferredLoader<'chip, C> {
        self.inner.loader()
    }
    fn squeeze_challenge(&mut self) -> DeferredScalar<'chip, C> {
        let scalar = self.inner.squeeze_challenge();
        self.challenges.push(*scalar.assigned());
        scalar
    }
    fn common_scalar(&mut self, scalar: &DeferredScalar<'chip, C>) -> Result<(), Error> {
        self.inner.common_scalar(scalar)
    }
    fn common_ec_point(&mut self, point: &DeferredEcPoint<'chip, C>) -> Result<(), Error> {
        self.inner.common_ec_point(point)
    }
}

impl<'chip, C, T> TranscriptRead<C, DeferredLoader<'chip, C>> for RecordingTranscript<'chip, C, T>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
    T: TranscriptRead<C, DeferredLoader<'chip, C>>,
{
    fn read_scalar(&mut self) -> Result<DeferredScalar<'chip, C>, Error> {
        self.inner.read_scalar()
    }
    fn read_ec_point(&mut self) -> Result<DeferredEcPoint<'chip, C>, Error> {
        self.inner.read_ec_point()
    }
}

fn fixture_key<C: CurveAffineExt>() -> IpaSuccinctVerifyingKey<C> {
    let point = |scalar| (C::generator() * C::ScalarExt::from(scalar)).to_affine();
    IpaSuccinctVerifyingKey::new(
        Domain::new(16, root_of_unity(16)),
        point(1),
        point(2),
        Some(point(3)),
    )
}

fn fixture_bytes<C: CurveAffineExt>() -> Vec<u8> {
    let mut bytes = Vec::new();
    let scalar = |bytes: &mut Vec<u8>, value| {
        bytes.extend_from_slice(C::ScalarExt::from(value).to_repr().as_ref());
    };
    let point = |bytes: &mut Vec<u8>, value| {
        bytes.extend_from_slice(
            (C::generator() * C::ScalarExt::from(value))
                .to_affine()
                .to_bytes()
                .as_ref(),
        );
    };
    scalar(&mut bytes, 1);
    scalar(&mut bytes, 2);
    point(&mut bytes, 3);
    scalar(&mut bytes, 4);
    point(&mut bytes, 5);
    scalar(&mut bytes, 6);
    for round in 0..16 {
        point(&mut bytes, 7 + 2 * round);
        point(&mut bytes, 8 + 2 * round);
    }
    point(&mut bytes, 41);
    scalar(&mut bytes, 42);
    assert_eq!(bytes.len(), KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1);
    bytes
}

fn drive_actual_fold<'chip, C, T>(
    key: &IpaSuccinctVerifyingKey<C>,
    inputs: &[DeferredAccumulator<'chip, C>],
    transcript: &mut T,
) -> Result<(), Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
    T: TranscriptRead<C, DeferredLoader<'chip, C>>,
{
    let proof = <IpaAs<C, Bgh19> as AccumulationScheme<C, DeferredLoader<'chip, C>>>::read_proof(
        key, inputs, transcript,
    )?;
    let _ = <IpaAs<C, Bgh19> as AccumulationScheme<C, DeferredLoader<'chip, C>>>::verify(
        key, inputs, &proof,
    )?;
    let _ = transcript.squeeze_challenge();
    Ok(())
}

struct Captured<C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    circuit: FoldTestCircuit<C::ScalarExt>,
    challenges: Vec<C::ScalarExt>,
    audit: crate::zk::pasta_cycle_loader::DeferredEquationWitness<C>,
}

fn capture<C>(native: bool, mutation: usize, wrapper: bool) -> Result<Captured<C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let mut bytes = fixture_bytes::<C>();
    match mutation {
        1 => bytes[..32].fill(0xff),
        2 => bytes[64..96].fill(0xff),
        3 => bytes[64..96].copy_from_slice(C::identity().to_bytes().as_ref()),
        4 => {
            bytes.pop();
        }
        5 => bytes.push(0),
        7 => bytes[64..96].copy_from_slice(
            (C::generator() * C::ScalarExt::from(49))
                .to_affine()
                .to_bytes()
                .as_ref(),
        ),
        8 => bytes[..32].copy_from_slice(C::ScalarExt::from(51).to_repr().as_ref()),
        _ => {}
    }
    let mut base = BaseCircuitBuilder::<C::ScalarExt>::new(false)
        .use_k(TEST_K)
        .use_lookup_bits(TEST_K - 1)
        .use_instance_columns(1);
    let range = base.range_chip();
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut base, &coordinate, &scalar_integer);
    let mut jobs = PastaNativePoseidonJobsV1::new(2, TEST_ROWS).map_err(transcript_error)?;
    let key = fixture_key::<C>();
    let mut inputs = Vec::new();
    for index in 0..2 {
        let mut xi = (0..16)
            .map(|i| C::ScalarExt::from((2 + 16 * index + i) as u64))
            .collect::<Vec<_>>();
        if mutation == 6 && index == 0 {
            xi[0] += C::ScalarExt::ONE;
        }
        let native_input = IpaAccumulator::<C, NativeLoader> {
            u: (C::generator() * C::ScalarExt::from((60 + index) as u64)).to_affine(),
            xi,
        };
        inputs.push(load_native_accumulator(&loader, &native_input)?);
    }
    let outputs = if wrapper {
        let mode = if native {
            ClaimFoldTranscriptModeV1::Native
        } else {
            ClaimFoldTranscriptModeV1::Base
        };
        vec![
            verify_claim_fold_with_transcript_binding_v1(
                &loader, &key, &inputs, &bytes, mode, &mut jobs,
            )?
            .transcript_binding,
        ]
    } else {
        let (reader, position) = ExactReader::new(&bytes);
        if native {
            let mut transcript = RecordingTranscript::<C, _>::new(ClaimNativeFoldTranscript::new(
                &loader, reader, &mut jobs,
            )?);
            drive_actual_fold(&key, &inputs, &mut transcript)?;
            if position.get() != bytes.len() {
                return Err(transcript_error("fold fixture has trailing bytes"));
            }
            let canonical = canonical_loaded_proof_bytes_v1(
                &loader,
                &transcript.inner.loaded_stream,
                KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1,
            )?;
            assert_eq!(
                canonical
                    .iter()
                    .copied()
                    .map(crate::zk::pasta_sha256::PastaSha256ByteV1::test_value)
                    .collect::<Vec<_>>(),
                bytes
            );
            let output = transcript.inner.finish()?;
            assert_eq!(output.cell, transcript.challenges.last().unwrap().cell);
            transcript.challenges
        } else {
            let mut transcript = RecordingTranscript::<C, _>::new(DeferredTranscript::new::<
                KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1,
            >(&loader, reader));
            drive_actual_fold(&key, &inputs, &mut transcript)?;
            if position.get() != bytes.len() {
                return Err(transcript_error("fold fixture has trailing bytes"));
            }
            let canonical = canonical_loaded_proof_bytes_v1(
                &loader,
                &transcript.inner.loaded_stream,
                KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1,
            )?;
            assert_eq!(
                canonical
                    .iter()
                    .copied()
                    .map(crate::zk::pasta_sha256::PastaSha256ByteV1::test_value)
                    .collect::<Vec<_>>(),
                bytes
            );
            transcript.challenges
        }
    };
    assert_eq!(outputs.len(), if wrapper { 1 } else { 21 });
    assert_eq!(
        jobs.required_rows().map_err(transcript_error)?,
        if native { 38 * 66 } else { 0 }
    );
    let challenges = outputs.iter().map(|cell| *cell.value()).collect();
    let audit = loader.ecc_chip().witness();
    *base.pool(0) = loader.take_ctx();
    base.assigned_instances = vec![outputs];
    super::super::super::base_packing::finalize_base_params_v1(&mut base, 9)
        .map_err(transcript_error)?;
    Ok(Captured {
        circuit: FoldTestCircuit { base, jobs },
        challenges,
        audit,
    })
}

#[test]
fn claim_native_fold_matches_every_original_challenge_byte_and_equation_in_both_fields() {
    fn check<C>()
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: KagemushaPoseidonFieldV1,
    {
        let old = capture::<C>(false, 0, false).unwrap();
        let native = capture::<C>(true, 0, false).unwrap();
        assert_eq!(old.challenges, native.challenges);
        assert_eq!(old.audit.sources, native.audit.sources);
        assert_eq!(old.audit.equations, native.audit.equations);
        assert_eq!(old.audit.equations.len(), 1);
        for captured in [&old, &native] {
            MockProver::run(
                TEST_K as u32,
                &captured.circuit,
                vec![old.challenges.clone()],
            )
            .unwrap()
            .assert_satisfied();
            let unknown = captured.circuit.without_witnesses();
            assert_eq!(
                format!("{:?}", unknown.params()),
                format!("{:?}", captured.circuit.params())
            );
            assert_eq!(
                unknown.jobs.required_rows(),
                captured.circuit.jobs.required_rows()
            );
        }
        for mode in [false, true] {
            let wrapped = capture::<C>(mode, 0, true).unwrap();
            assert_eq!(wrapped.challenges, vec![*old.challenges.last().unwrap()]);
            assert_eq!(wrapped.audit.sources, old.audit.sources);
            assert_eq!(wrapped.audit.equations, old.audit.equations);
            MockProver::run(
                TEST_K as u32,
                &wrapped.circuit,
                vec![wrapped.challenges.clone()],
            )
            .unwrap()
            .assert_satisfied();
        }
    }
    check::<EqAffine>();
    check::<EpAffine>();
}

#[test]
fn claim_native_fold_preserves_canonical_reader_rejections_and_input_bindings_in_both_fields() {
    fn check<C>()
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: KagemushaPoseidonFieldV1,
    {
        for mutation in 1..=5 {
            for wrapper in [false, true] {
                let old = capture::<C>(false, mutation, wrapper)
                    .err()
                    .expect("original reader rejects");
                let native = capture::<C>(true, mutation, wrapper)
                    .err()
                    .expect("native reader rejects");
                assert_eq!(format!("{old:?}"), format!("{native:?}"));
            }
        }
        let original = capture::<C>(false, 0, true).unwrap();
        for mutation in [6, 7, 8] {
            let old = capture::<C>(false, mutation, true).unwrap();
            let native = capture::<C>(true, mutation, true).unwrap();
            assert_eq!(old.challenges, native.challenges);
            assert_ne!(old.challenges, original.challenges);
            for captured in [&old, &native] {
                assert!(
                    MockProver::run(
                        TEST_K as u32,
                        &captured.circuit,
                        vec![original.challenges.clone()]
                    )
                    .unwrap()
                    .verify()
                    .is_err(),
                    "common scalar/proof point/proof scalar mutation {mutation}"
                );
            }
        }
    }
    check::<EqAffine>();
    check::<EpAffine>();
}

#[test]
fn claim_fold_geometry_preserves_large_seeds_and_reserves_complete_folds() {
    use ClaimFoldTranscriptModeV1::{Base, Native};
    assert_eq!(
        FOLD_SQUEEZE_INPUTS
            .iter()
            .map(|count| count / 2 + 1)
            .sum::<usize>(),
        75
    );
    for (m, expected) in [
        (760, [Native, Native]),
        (766, [Native, Native]),
        (767, [Native, Base]),
        (841, [Native, Base]),
        (842, [Base, Base]),
        (874, [Base, Base]),
        (875, [Base, Base]),
    ] {
        let plan = ClaimFoldTranscriptPlanV1::new(m - 1, 1).unwrap();
        assert_eq!([plan.parent(), plan.successor()], expected);
        let native_count = expected.iter().filter(|&&mode| mode == Native).count();
        assert_eq!(plan.native_permutation_count(), native_count * 75);
        if m <= 874 {
            assert!(MAX_BATCH_PERMUTATIONS + m + 14 + native_count * 75 <= NATIVE_CAPACITY);
        }
        assert_eq!(plan, ClaimFoldTranscriptPlanV1::new(1, m - 1).unwrap());
    }
    assert!(ClaimFoldTranscriptPlanV1::new(usize::MAX, 1).is_err());
    assert!(ClaimFoldTranscriptPlanV1::new(usize::MAX - 1, 0).is_err());
}

#[test]
fn claim_fold_reservations_fit_with_all_mandatory_hashes_at_mode_boundaries_in_both_fields() {
    fn check<F: KagemushaPoseidonFieldV1>() {
        let rows = (1 << 16) - 9;
        let gate = halo2_base::gates::GateChip::default();
        for (sources, equations, m, expected_rows) in [
            // A nonmaximal two-fold queue reproduces the full State diagnostic's
            // 20,394-row inventory. Omitting its 150 fold permutations gives 15,444.
            (373, 7, 35, 20_394),
            (1_008, 7, 766, 65_472),
            (1_008, 7, 841, 65_472),
            (1_008, 7, 874, 64_086),
        ] {
            let mut base = BaseCircuitBuilder::<F>::new(false).use_k(16);
            let mut jobs = PastaNativePoseidonJobsV1::new(2, rows).unwrap();
            let plan = ClaimFoldTranscriptPlanV1::new(m - 1, 1).unwrap();
            // Geometry only: reserve the stated source batch and two nonempty
            // identities. These synthetic hash messages do not represent an accepted Claim.
            for count in [77 + 2 * sources + 2 * equations, 12 + 2 * (m - 1), 14] {
                let cells = (0..count)
                    .map(|i| base.main(0).load_witness(F::from(i as u64)))
                    .collect();
                jobs.queue_raw(base.main(0), &gate, cells, &[]).unwrap();
            }
            for mode in [plan.parent(), plan.successor()] {
                if mode == ClaimFoldTranscriptModeV1::Native {
                    let mut reservation = jobs
                        .begin_transcript(base.main(0), &FOLD_SQUEEZE_INPUTS)
                        .unwrap();
                    for count in FOLD_SQUEEZE_INPUTS {
                        let inputs = (0..count)
                            .map(|i| base.main(0).load_witness(F::from(i as u64)))
                            .collect::<Vec<_>>();
                        reservation.absorb(&inputs).unwrap();
                        let _ = reservation.squeeze(base.main(0), &gate);
                    }
                    reservation.finish().unwrap();
                }
            }
            assert_eq!(jobs.required_rows(), Ok(expected_rows));
            assert_eq!(
                jobs.required_rows().unwrap(),
                (39 + sources + equations + m + 14 + plan.native_permutation_count()).div_ceil(2)
                    * 66,
            );
            assert_eq!(jobs.clone().unknown().required_rows(), jobs.required_rows());
        }
    }
    check::<halo2_proofs::halo2curves::pasta::Fp>();
    check::<halo2_proofs::halo2curves::pasta::Fq>();
}
