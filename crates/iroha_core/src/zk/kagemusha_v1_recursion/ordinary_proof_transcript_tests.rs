//! Complete Base/native ordinary-reader comparisons in both Pasta fields.
//!
//! Hybrid parser fixtures exercise every verifier operation but are not valid monetary proofs.
//! Separate small ordinary proofs use the real Halo2 IPA prover and native verification before
//! entering this scalar-half verifier. Complete Claim convergence and phone gates remain separate.

use super::*;
use crate::zk::kagemusha_v1_recursion::deferred_parent::ordinary_poseidon_schedule::ordinary_poseidon_squeeze_inputs_v1;
use crate::zk::pasta_native_poseidon::PastaNativePoseidonConfigV1;
use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig};
use halo2_proofs::{
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::{
        group::{Curve as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine},
    },
    plonk::{Circuit, ConstraintSystem, Error as Halo2Error, keygen_vk},
    poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
};
use snark_verifier::{
    system::halo2::{Config, compile},
    util::arithmetic::{Domain, root_of_unity},
};

const TEST_K: usize = 13;
const TEST_ROWS: usize = (1 << TEST_K) - 9;

#[derive(Clone, Debug)]
struct OrdinaryTestConfig<F: KagemushaPoseidonFieldV1> {
    base: BaseConfig<F>,
    native: PastaNativePoseidonConfigV1,
}

#[derive(Clone)]
struct OrdinaryTestCircuit<F: KagemushaPoseidonFieldV1> {
    base: BaseCircuitBuilder<F>,
    jobs: PastaNativePoseidonJobsV1<F>,
}

impl<F: KagemushaPoseidonFieldV1> Circuit<F> for OrdinaryTestCircuit<F> {
    type Config = OrdinaryTestConfig<F>;
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
        unreachable!("ordinary transcript test uses parameters")
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows(TEST_ROWS);
        OrdinaryTestConfig {
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
        self.base.synthesize(
            config.base,
            layouter.namespace(|| "ordinary transcript Base"),
        )?;
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
    challenges: Rc<std::cell::RefCell<Vec<AssignedValue<C::ScalarExt>>>>,
    marker: std::marker::PhantomData<&'chip C>,
}

impl<'chip, C, T> RecordingTranscript<'chip, C, T>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    fn new(inner: T, challenges: Rc<std::cell::RefCell<Vec<AssignedValue<C::ScalarExt>>>>) -> Self {
        Self {
            inner,
            challenges,
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
        self.challenges.borrow_mut().push(*scalar.assigned());
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

impl<'chip, C, T> CompleteProofTranscriptV1<'chip, C> for RecordingTranscript<'chip, C, T>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
    T: CompleteProofTranscriptV1<'chip, C>,
{
    fn finish_stream(self) -> Result<DeferredProofStreamV1<'chip, C>, Error> {
        self.inner.finish_stream()
    }
}

struct OrdinaryFixture<C: CurveAffineExt> {
    protocol: PlonkProtocol<C>,
    key: IpaSuccinctVerifyingKey<C>,
    instances: Vec<Vec<C::ScalarExt>>,
    bytes: Vec<u8>,
    hybrid: bool,
    first_witness_offset: usize,
    first_scalar_offset: usize,
}

fn point<C: CurveAffineExt>(value: u64) -> C {
    (C::generator() * C::ScalarExt::from(value)).to_affine()
}

fn compressed_limbs<C: CurveAffineExt>(point: C) -> [C::ScalarExt; 2] {
    let encoded = point.to_bytes();
    let bytes = encoded.as_ref();
    assert_eq!(bytes.len(), 32);
    std::array::from_fn(|half| {
        C::ScalarExt::from_u128(u128::from_le_bytes(
            bytes[16 * half..16 * (half + 1)].try_into().unwrap(),
        ))
    })
}

fn hybrid_shape_fixture<C>() -> OrdinaryFixture<C>
where
    C: CurveAffineExt,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let k = KAGEMUSHA_RECURSION_IPA_K_V1 as usize;
    let carriers = [point::<C>(31), point::<C>(32)];
    let mut semantic = vec![C::ScalarExt::from(11), C::ScalarExt::from(12)];
    semantic.extend(carriers.into_iter().flat_map(compressed_limbs::<C>));
    let instances = vec![
        semantic,
        vec![C::ScalarExt::from(13); 7],
        vec![C::ScalarExt::from(14); 8],
    ];
    let mut builder = BaseCircuitBuilder::<C::ScalarExt>::new(false)
        .use_k(k)
        .use_instance_columns(3);
    let gate = halo2_base::gates::GateChip::default();
    let x = builder.main(0).load_witness(C::ScalarExt::from(11));
    let _ = gate.add(builder.main(0), x, x);
    builder.assigned_instances = instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|value| builder.main(0).load_witness(*value))
                .collect()
        })
        .collect();
    builder.calculate_params(Some(9));
    let params = ParamsIPA::<C>::new(k as u32);
    let vk = keygen_vk(&params, &builder).expect("actual tiny three-column hybrid fixture VK");
    let mut protocol = compile(&params, &vk, Config::ipa().with_num_instance(vec![6, 7, 8]));
    // The real hybrid reader loads only the semantic ICK bases. Wide columns retain their
    // full authenticated counts and opening queries, with proof-supplied commitments.
    protocol
        .instance_committing_key
        .as_mut()
        .expect("IPA commits public columns")
        .bases
        .truncate(6);
    let profile = ordinary_ipa_proof_profile_v1(&protocol).unwrap();
    assert!(profile.witness_commitments > 0 && profile.evaluations > 0);
    let mut bytes = Vec::new();
    for carrier in carriers {
        bytes.extend_from_slice(carrier.to_bytes().as_ref());
    }
    let first_witness_offset = bytes.len();
    for index in 0..profile.witness_commitments + profile.quotient_commitments {
        bytes.extend_from_slice(point::<C>(40 + index as u64).to_bytes().as_ref());
    }
    let first_scalar_offset = bytes.len();
    for index in 0..profile.evaluations {
        bytes.extend_from_slice(C::ScalarExt::from(80 + index as u64).to_repr().as_ref());
    }
    bytes.extend_from_slice(point::<C>(100).to_bytes().as_ref());
    for index in 0..profile.bgh19_rotation_sets {
        bytes.extend_from_slice(C::ScalarExt::from(110 + index as u64).to_repr().as_ref());
    }
    bytes.extend_from_slice(point::<C>(120).to_bytes().as_ref());
    for index in 0..2 * k {
        bytes.extend_from_slice(point::<C>(130 + index as u64).to_bytes().as_ref());
    }
    for value in [170, 171] {
        bytes.extend_from_slice(C::ScalarExt::from(value).to_repr().as_ref());
    }
    bytes.extend_from_slice(point::<C>(172).to_bytes().as_ref());
    assert_eq!(bytes.len(), profile.byte_len + 64);
    let key = IpaSuccinctVerifyingKey::new(
        Domain::new(k, root_of_unity(k)),
        point::<C>(1),
        point::<C>(2),
        Some(point::<C>(3)),
    );
    OrdinaryFixture {
        protocol,
        key,
        instances,
        bytes,
        hybrid: true,
        first_witness_offset,
        first_scalar_offset,
    }
}

fn genuine_small_ordinary_fixture<C>() -> OrdinaryFixture<C>
where
    C: CurveAffineExt,
    C::ScalarExt: KagemushaPoseidonFieldV1 + ff::WithSmallOrderMulGroup<3>,
{
    use crate::zk::kagemusha_v1_recursion::generation::{
        KagemushaRawHalo2IpaProofV1, augment_halo2_ipa_proof_v1,
    };
    use halo2_proofs::{
        halo2curves::CurveExt as _,
        plonk::{create_proof, keygen_pk, verify_proof},
        poly::{
            VerificationStrategy as _,
            commitment::Params as _,
            ipa::{
                commitment::IPACommitmentScheme,
                multiopen::{ProverIPA, VerifierIPA},
                strategy::SingleStrategy,
            },
        },
    };
    use rand_core_06::OsRng;
    use snark_verifier::system::halo2::transcript::halo2::ChallengeScalar;
    let k = 6;
    let mut builder = BaseCircuitBuilder::<C::ScalarExt>::new(false)
        .use_k(k)
        .use_instance_columns(1);
    let gate = halo2_base::gates::GateChip::default();
    let x = builder.main(0).load_witness(C::ScalarExt::from(11));
    let y = builder.main(0).load_witness(C::ScalarExt::from(12));
    let z = gate.add(builder.main(0), x, y);
    builder.assigned_instances = vec![vec![x, y, z]];
    builder.calculate_params(Some(9));
    let params = ParamsIPA::<C>::new(k as u32);
    let vk = keygen_vk(&params, &builder).expect("genuine small ordinary VK");
    let pk = keygen_pk(&params, vk, &builder).expect("genuine small ordinary PK");
    let protocol = compile(
        &params,
        pk.get_vk(),
        Config::ipa().with_num_instance(vec![3]),
    );
    let instances = vec![vec![
        C::ScalarExt::from(11),
        C::ScalarExt::from(12),
        C::ScalarExt::from(23),
    ]];
    let columns = [instances[0].as_slice()];
    let mut transcript = PoseidonTranscript::<
        C,
        NativeLoader,
        _,
        KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
        KAGEMUSHA_IPA_POSEIDON_RATE_V1,
        KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
        KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    >::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::new());
    create_proof::<IPACommitmentScheme<C>, ProverIPA<'_, C>, ChallengeScalar<C>, _, _, _>(
        &params,
        &pk,
        &[builder],
        &[&columns],
        OsRng,
        &mut transcript,
    )
    .expect("produce a genuine ordinary IPA proof");
    let raw = transcript.finalize();
    let verify = |public: &[C::ScalarExt], proof: &[u8]| {
        let columns = [public];
        let mut transcript = PoseidonTranscript::<
            C,
            NativeLoader,
            _,
            KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
            KAGEMUSHA_IPA_POSEIDON_RATE_V1,
            KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
            KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
        >::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(proof);
        verify_proof::<IPACommitmentScheme<C>, VerifierIPA<'_, C>, ChallengeScalar<C>, _, _>(
            &params,
            pk.get_vk(),
            SingleStrategy::<C>::new(&params),
            &[&columns],
            &mut transcript,
        )
    };
    verify(&instances[0], &raw).expect("native verifier checks the complete real IPA equation");
    let mut changed_public = instances[0].clone();
    changed_public[0] += C::ScalarExt::ONE;
    assert!(verify(&changed_public, &raw).is_err());
    let mut changed_proof = raw.clone();
    changed_proof[..32].fill(0xff);
    assert!(verify(&instances[0], &changed_proof).is_err());
    let bytes = augment_halo2_ipa_proof_v1(
        &params,
        pk.get_vk(),
        KagemushaRawHalo2IpaProofV1::new(raw),
        &instances[0],
    )
    .expect("append the exact transcript-derived folded generator after full verification");
    let profile = ordinary_ipa_proof_profile_at_k_v1(&protocol, k).unwrap();
    assert_eq!(bytes.len(), profile.byte_len);
    let hash_to_curve = C::CurveExt::hash_to_curve("Halo2-Parameters");
    let key = IpaSuccinctVerifyingKey::new(
        Domain::new(k, root_of_unity(k)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    );
    let first_scalar_offset = 32 * (profile.witness_commitments + profile.quotient_commitments);
    OrdinaryFixture {
        protocol,
        key,
        instances,
        bytes,
        hybrid: false,
        first_witness_offset: 0,
        first_scalar_offset,
    }
}

struct Captured<C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    circuit: OrdinaryTestCircuit<C::ScalarExt>,
    challenges: Vec<C::ScalarExt>,
    accumulator: Vec<C::ScalarExt>,
    canonical: Vec<u8>,
    audit: crate::zk::pasta_cycle_loader::DeferredEquationWitness<C>,
}

fn capture<C>(
    fixture: &OrdinaryFixture<C>,
    native: bool,
    mutation: usize,
) -> Result<Captured<C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let mut bytes = fixture.bytes.clone();
    let mut instances = fixture.instances.clone();
    let point_offset = fixture.first_witness_offset;
    let scalar_offset = fixture.first_scalar_offset;
    match mutation {
        1 => bytes[scalar_offset..scalar_offset + 32].fill(0xff),
        2 => bytes[point_offset..point_offset + 32].fill(0xff),
        3 => bytes[point_offset..point_offset + 32]
            .copy_from_slice(C::identity().to_bytes().as_ref()),
        4 => {
            bytes.pop();
        }
        5 => bytes.push(0),
        6 => instances[0][0] += C::ScalarExt::ONE,
        7 => bytes[point_offset..point_offset + 32]
            .copy_from_slice(point::<C>(199).to_bytes().as_ref()),
        8 => bytes[scalar_offset..scalar_offset + 32]
            .copy_from_slice(C::ScalarExt::from(201).to_repr().as_ref()),
        9 if fixture.hybrid => {
            let first = bytes[..32].to_vec();
            let second = bytes[32..64].to_vec();
            bytes[..32].copy_from_slice(&second);
            bytes[32..64].copy_from_slice(&first);
        }
        _ => {}
    }
    let mut base = BaseCircuitBuilder::<C::ScalarExt>::new(false)
        .use_k(TEST_K)
        .use_lookup_bits(TEST_K - 1)
        .use_instance_columns(1);
    let range = base.range_chip();
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut base, &coordinate, &scalar_integer);
    let protocol = fixture
        .protocol
        .loaded_preprocessed_as_witness(&loader, false);
    let instances = instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|value| loader.assign_scalar(*value))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let schedule =
        ordinary_poseidon_squeeze_inputs_v1(&fixture.protocol, fixture.protocol.domain.k)
            .map_err(transcript_error)?;
    let mut jobs = PastaNativePoseidonJobsV1::new(2, TEST_ROWS).map_err(transcript_error)?;
    let challenges = Rc::new(std::cell::RefCell::new(Vec::new()));
    let (accumulator, stream, binding) = if fixture.hybrid {
        let parsed = if native {
            let recorded = Rc::clone(&challenges);
            let reserved_jobs = &mut jobs;
            let native_schedule = schedule.as_slice();
            verify_multi_carrier_hybrid_ordinary_proof_and_stream_with_factory_v1(
                &loader,
                &fixture.key,
                &protocol,
                &instances[0],
                [[2, 3], [4, 5]],
                &bytes,
                move |loader, reader| {
                    Ok(RecordingTranscript::new(
                        NativeOrdinaryTranscriptV1::new(
                            loader,
                            reader,
                            reserved_jobs,
                            native_schedule,
                        )?,
                        recorded,
                    ))
                },
            )?
        } else {
            let recorded = Rc::clone(&challenges);
            verify_multi_carrier_hybrid_ordinary_proof_and_stream_with_factory_v1(
                &loader,
                &fixture.key,
                &protocol,
                &instances[0],
                [[2, 3], [4, 5]],
                &bytes,
                |loader, reader| {
                    Ok(RecordingTranscript::new(
                        DeferredTranscript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(
                            loader, reader,
                        ),
                        recorded,
                    ))
                },
            )?
        };
        (
            parsed.accumulator,
            parsed.loaded_stream,
            parsed.transcript_binding,
        )
    } else if native {
        let recorded = Rc::clone(&challenges);
        let reserved_jobs = &mut jobs;
        let native_schedule = schedule.as_slice();
        verify_ordinary_proof_and_stream_at_k_with_factory_v1(
            &loader,
            &fixture.key,
            &protocol,
            &instances,
            &bytes,
            fixture.protocol.domain.k,
            move |loader, reader| {
                Ok(RecordingTranscript::new(
                    NativeOrdinaryTranscriptV1::new(
                        loader,
                        reader,
                        reserved_jobs,
                        native_schedule,
                    )?,
                    recorded,
                ))
            },
        )?
    } else {
        let recorded = Rc::clone(&challenges);
        verify_ordinary_proof_and_stream_at_k_with_factory_v1(
            &loader,
            &fixture.key,
            &protocol,
            &instances,
            &bytes,
            fixture.protocol.domain.k,
            |loader, reader| {
                Ok(RecordingTranscript::new(
                    DeferredTranscript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(loader, reader),
                    recorded,
                ))
            },
        )?
    };
    let canonical = canonical_loaded_proof_bytes_v1(&loader, &stream, bytes.len())?
        .into_iter()
        .map(crate::zk::pasta_sha256::PastaSha256ByteV1::test_value)
        .collect::<Vec<_>>();
    assert_eq!(canonical, bytes);
    let outputs = challenges.borrow().clone();
    assert_eq!(outputs.len(), schedule.len());
    assert_eq!(outputs.last().unwrap().cell, binding.cell);
    let accumulator = {
        let chip = loader.ecc_chip();
        let mut ctx = loader.ctx_mut();
        let u = chip.assigned_point_poseidon_elements_v1(&mut ctx, &accumulator.u.assigned())?;
        u.into_iter()
            .map(|value| *value.value())
            .chain(accumulator.xi.iter().map(|value| *value.assigned().value()))
            .collect()
    };
    let audit = loader.ecc_chip().witness();
    let challenge_values = outputs.iter().map(|value| *value.value()).collect();
    *base.pool(0) = loader.take_ctx();
    base.assigned_instances = vec![outputs];
    crate::zk::kagemusha_v1_recursion::base_packing::finalize_base_params_v1(&mut base, 9)
        .map_err(transcript_error)?;
    let permutations = schedule.iter().map(|count| count / 2 + 1).sum::<usize>();
    assert_eq!(
        jobs.required_rows().map_err(transcript_error)?,
        if native {
            permutations.div_ceil(2) * 66
        } else {
            0
        }
    );
    Ok(Captured {
        circuit: OrdinaryTestCircuit { base, jobs },
        challenges: challenge_values,
        accumulator,
        canonical,
        audit,
    })
}

fn capture_public_entrypoint<C>(fixture: &OrdinaryFixture<C>, native: bool) -> Captured<C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let mut base = BaseCircuitBuilder::<C::ScalarExt>::new(false)
        .use_k(TEST_K)
        .use_lookup_bits(TEST_K - 1)
        .use_instance_columns(1);
    let range = base.range_chip();
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut base, &coordinate, &scalar_integer);
    let protocol = fixture
        .protocol
        .loaded_preprocessed_as_witness(&loader, false);
    let instances = fixture
        .instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|value| loader.assign_scalar(*value))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let schedule =
        ordinary_poseidon_squeeze_inputs_v1(&fixture.protocol, fixture.protocol.domain.k)
            .expect("authenticated public-entrypoint schedule");
    let selected = native.then_some(schedule.as_slice());
    let mut jobs = PastaNativePoseidonJobsV1::new(2, TEST_ROWS).unwrap();
    let (accumulator, binding, canonical) = if fixture.hybrid {
        let parsed = verify_two_carrier_hybrid_ordinary_proof_with_native_v1(
            &loader,
            &fixture.key,
            &protocol,
            &instances[0],
            [[2, 3], [4, 5]],
            &fixture.bytes,
            selected,
            &mut jobs,
        )
        .expect("complete two-carrier public entrypoint");
        let canonical =
            canonical_loaded_proof_bytes_v1(&loader, &parsed.loaded_stream, fixture.bytes.len())
                .unwrap()
                .into_iter()
                .map(crate::zk::pasta_sha256::PastaSha256ByteV1::test_value)
                .collect();
        (parsed.accumulator, parsed.transcript_binding, canonical)
    } else {
        let (accumulator, binding) = verify_ordinary_proof_with_native_binding_at_k_v1(
            &loader,
            &fixture.key,
            &protocol,
            &instances,
            &fixture.bytes,
            fixture.protocol.domain.k,
            selected,
            &mut jobs,
        )
        .expect("complete ordinary public entrypoint");
        (accumulator, binding, Vec::new())
    };
    let accumulator = {
        let chip = loader.ecc_chip();
        let mut ctx = loader.ctx_mut();
        let u = chip
            .assigned_point_poseidon_elements_v1(&mut ctx, &accumulator.u.assigned())
            .unwrap();
        u.into_iter()
            .map(|value| *value.value())
            .chain(accumulator.xi.iter().map(|value| *value.assigned().value()))
            .collect()
    };
    let audit = loader.ecc_chip().witness();
    let challenges = vec![*binding.value()];
    *base.pool(0) = loader.take_ctx();
    base.assigned_instances = vec![vec![binding]];
    crate::zk::kagemusha_v1_recursion::base_packing::finalize_base_params_v1(&mut base, 9).unwrap();
    let permutations = schedule.iter().map(|count| count / 2 + 1).sum::<usize>();
    assert_eq!(
        jobs.required_rows().unwrap(),
        if native {
            permutations.div_ceil(2) * 66
        } else {
            0
        }
    );
    Captured {
        circuit: OrdinaryTestCircuit { base, jobs },
        challenges,
        accumulator,
        canonical,
        audit,
    }
}

fn assert_complete_equivalence<C>(fixture: &OrdinaryFixture<C>)
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let old = capture(fixture, false, 0).expect("complete original ordinary verifier");
    let native = capture(fixture, true, 0).expect("complete native ordinary verifier");
    assert_eq!(old.challenges, native.challenges);
    assert_eq!(old.accumulator, native.accumulator);
    assert_eq!(old.canonical, native.canonical);
    assert_eq!(old.audit.sources, native.audit.sources);
    assert_eq!(old.audit.equations, native.audit.equations);
    assert!(!old.audit.equations.is_empty());
    // Exercise the actual Claim/helper entrypoints, including their preselected
    // Base branch. The ordinary binding-only API intentionally drops the stream;
    // the recording factory above compares its complete original read stream.
    for selected_native in [false, true] {
        let entry = capture_public_entrypoint(fixture, selected_native);
        assert_eq!(
            entry.challenges.as_slice(),
            &old.challenges[old.challenges.len() - 1..]
        );
        assert_eq!(entry.accumulator, old.accumulator);
        assert_eq!(entry.audit.sources, old.audit.sources);
        assert_eq!(entry.audit.equations, old.audit.equations);
        if fixture.hybrid {
            assert_eq!(entry.canonical, old.canonical);
        }
        MockProver::run(
            TEST_K as u32,
            &entry.circuit,
            vec![entry.challenges.clone()],
        )
        .expect("public-entrypoint transcript binding circuit")
        .assert_satisfied();
    }
    for captured in [&old, &native] {
        MockProver::run(
            TEST_K as u32,
            &captured.circuit,
            vec![old.challenges.clone()],
        )
        .expect("complete ordinary transcript circuit")
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
    // Every squeeze is an original assigned public cell, including the final binding.
    // Changing an intermediate challenge or the final binding cannot be hidden by a later squeeze.
    for index in [0, native.challenges.len() / 2, native.challenges.len() - 1] {
        let mut changed = native.challenges.clone();
        changed[index] += C::ScalarExt::ONE;
        assert!(
            MockProver::run(TEST_K as u32, &native.circuit, vec![changed])
                .expect("altered ordinary transcript challenge")
                .verify()
                .is_err()
        );
    }
}

fn assert_reader_rejections_and_bindings<C>(fixture: &OrdinaryFixture<C>)
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    for mutation in 1..=5 {
        let old = capture(fixture, false, mutation)
            .err()
            .expect("original reader rejects");
        let native = capture(fixture, true, mutation)
            .err()
            .expect("native reader rejects");
        assert_eq!(format!("{old:?}"), format!("{native:?}"));
    }
    let original = capture(fixture, false, 0).expect("original transcript binding");
    for mutation in [6, 7, 8] {
        let old = capture(fixture, false, mutation).expect("canonical changed transcript");
        let native = capture(fixture, true, mutation).expect("canonical changed native transcript");
        assert_eq!(old.challenges, native.challenges);
        assert_ne!(native.challenges, original.challenges);
        assert_eq!(old.accumulator, native.accumulator);
        assert_eq!(old.audit.sources, native.audit.sources);
        assert_eq!(old.audit.equations, native.audit.equations);
        for captured in [&old, &native] {
            assert!(
                MockProver::run(
                    TEST_K as u32,
                    &captured.circuit,
                    vec![original.challenges.clone()]
                )
                .expect("changed proof/public-input binding")
                .verify()
                .is_err()
            );
        }
    }
    if fixture.hybrid {
        for native in [false, true] {
            let changed =
                capture(fixture, native, 9).expect("swapped canonical carrier points parse");
            assert!(
                MockProver::run(
                    TEST_K as u32,
                    &changed.circuit,
                    vec![changed.challenges.clone()]
                )
                .expect("swapped complete hybrid carrier commitments")
                .verify()
                .is_err(),
                "carrier order must remain constrained to the original semantic limbs"
            );
        }
    }
}

fn assert_native_abort_poisoning<C>()
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    // Each case starts a fresh circuit/queue; an abandoned queue may not be reused.
    for case in 0..4 {
        let mut base = BaseCircuitBuilder::<C::ScalarExt>::new(false)
            .use_k(TEST_K)
            .use_lookup_bits(TEST_K - 1);
        let range = base.range_chip();
        let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
        let loader = deferred_loader_v1(&mut base, &coordinate, &scalar_integer);
        let mut jobs = PastaNativePoseidonJobsV1::new(2, TEST_ROWS).unwrap();
        let bytes = if case == 0 {
            vec![0xff; 32]
        } else {
            C::ScalarExt::from(11).to_repr().as_ref().to_vec()
        };
        {
            let schedule = if case == 2 { vec![0] } else { vec![1, 0] };
            let (reader, _) = ExactReader::new(&bytes);
            let mut transcript =
                NativeOrdinaryTranscriptV1::<C, _>::new(&loader, reader, &mut jobs, &schedule)
                    .expect("reserve the exact native transcript test schedule");
            match case {
                0 => {
                    assert!(transcript.read_scalar().is_err());
                }
                1 => {
                    transcript.read_scalar().unwrap();
                    let _ = transcript.squeeze_challenge();
                }
                2 => {
                    assert!(transcript.read_scalar().is_err());
                    assert!(transcript.finish_stream().is_err());
                }
                3 => {
                    transcript.read_scalar().unwrap();
                    let _ = transcript.squeeze_challenge();
                    assert!(transcript.finish_stream().is_err());
                }
                _ => unreachable!(),
            }
        }
        assert!(
            jobs.required_rows().is_err(),
            "case {case} cannot discard an unfinished reservation"
        );
        let (reader, _) = ExactReader::new(&bytes);
        assert!(
            NativeOrdinaryTranscriptV1::<C, _>::new(&loader, reader, &mut jobs, &[1]).is_err(),
            "case {case} cannot replace its abandoned native constraints with a fresh transcript"
        );
    }
}

#[test]
fn ordinary_native_matches_real_small_ipa_in_both_fields() {
    fn check<C>()
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: KagemushaPoseidonFieldV1 + ff::WithSmallOrderMulGroup<3>,
    {
        let fixture = genuine_small_ordinary_fixture::<C>();
        assert_complete_equivalence(&fixture);
        assert_reader_rejections_and_bindings(&fixture);
    }
    check::<EqAffine>();
    check::<EpAffine>();
}

#[test]
fn hybrid_native_matches_complete_k16_verifier_and_carrier_bindings_in_both_fields() {
    fn check<C>()
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: KagemushaPoseidonFieldV1,
    {
        let fixture = hybrid_shape_fixture::<C>();
        assert_complete_equivalence(&fixture);
        assert_reader_rejections_and_bindings(&fixture);
    }
    check::<EqAffine>();
    check::<EpAffine>();
}

#[test]
fn ordinary_native_rejects_malformed_or_abandoned_reservations_in_both_fields() {
    assert_native_abort_poisoning::<EqAffine>();
    assert_native_abort_poisoning::<EpAffine>();
}
