//! Reference, binding and source-inventory tests for Claim-native protocol identities.

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
    poly::commitment::ParamsProver as _,
};
use snark_verifier::{
    system::halo2::{Config as ProtocolConfig, compile},
    util::arithmetic::{Domain, root_of_unity},
};

const TEST_K: usize = 12;
const TEST_ROWS: usize = (1 << TEST_K) - 9;

#[derive(Clone, Debug)]
struct IdentityConfig<F: KagemushaPoseidonFieldV1> {
    base: BaseConfig<F>,
    native: PastaNativePoseidonConfigV1,
}

#[derive(Clone)]
struct IdentityCircuit<F: KagemushaPoseidonFieldV1> {
    base: BaseCircuitBuilder<F>,
    jobs: PastaNativePoseidonJobsV1<F>,
}

impl<F: KagemushaPoseidonFieldV1> Circuit<F> for IdentityCircuit<F> {
    type Config = IdentityConfig<F>;
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
        unreachable!("protocol identity uses parameterized Base configuration")
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows(TEST_ROWS);
        IdentityConfig {
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
            .synthesize(config.base, layouter.namespace(|| "protocol identity Base"))?;
        self.jobs.synthesize(
            &config.native,
            &mut layouter,
            &self.base.core().copy_manager,
            self.base.witness_gen_only(),
            TEST_ROWS,
        )
    }
}

fn protocol<C>() -> PlonkProtocol<C>
where
    C: CurveAffineExt,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    // This is a value/structure-identity fixture, not a proof or a release key. Repeated
    // preprocessed values deliberately test the source allocation used by the capacity proof.
    // Obtain the private quotient type through the public protocol compiler. The fixture
    // uses a constant-zero numerator and the same chunk degree; no proof is claimed.
    let mut seed = BaseCircuitBuilder::<C::ScalarExt>::new(false).use_k(4);
    seed.main(0).load_constant(C::ScalarExt::ONE);
    seed.calculate_params(Some(9));
    let parameters = halo2_proofs::poly::ipa::commitment::ParamsIPA::<C>::new(4);
    let key = halo2_proofs::plonk::keygen_vk(&parameters, &seed).expect("tiny protocol seed key");
    let mut quotient = compile(&parameters, &key, ProtocolConfig::ipa()).quotient;
    quotient.chunk_degree = 1;
    quotient.numerator = std::iter::empty().sum();
    let generator = C::generator();
    PlonkProtocol {
        domain: Domain::new(16, root_of_unity::<C::ScalarExt>(16)),
        domain_as_witness: None,
        preprocessed: vec![
            generator,
            (generator * C::ScalarExt::from(2)).to_affine(),
            generator,
        ],
        num_instance: vec![2],
        num_witness: vec![1],
        num_challenge: vec![0],
        evaluations: Vec::new(),
        queries: Vec::new(),
        quotient,
        transcript_initial_state: Some(C::ScalarExt::from(17)),
        instance_committing_key: None,
        linearization: None,
        accumulator_indices: Vec::new(),
    }
}

fn identity_fixture<C>(
    parity: KagemushaPastaParityV1,
    native: bool,
    mutation: usize,
) -> Result<(IdentityCircuit<C::ScalarExt>, Vec<Vec<C::ScalarExt>>), Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let mut protocol = protocol::<C>();
    let expected = native_parent_protocol_digest_v1(&protocol, parity).map_err(transcript_error)?;
    let mut expected_values = [
        C::ScalarExt::from_u128(u128::from_le_bytes(expected[..16].try_into().unwrap())),
        C::ScalarExt::from_u128(u128::from_le_bytes(expected[16..].try_into().unwrap())),
    ];
    let actual_parity = if mutation == 5 {
        match parity {
            KagemushaPastaParityV1::Eq => KagemushaPastaParityV1::Ep,
            KagemushaPastaParityV1::Ep => KagemushaPastaParityV1::Eq,
        }
    } else {
        parity
    };
    match mutation {
        1 => protocol.preprocessed[0] = (C::generator() * C::ScalarExt::from(3)).to_affine(),
        2 => protocol.preprocessed.swap(0, 1),
        3 => protocol.transcript_initial_state = Some(C::ScalarExt::from(18)),
        4 => protocol.preprocessed.push(C::generator()),
        6 => expected_values[0] += C::ScalarExt::ONE,
        8 => protocol.num_instance[0] += 1,
        9 => protocol.domain = Domain::new(15, root_of_unity::<C::ScalarExt>(15)),
        10 => protocol.transcript_initial_state = None,
        11 => protocol.preprocessed[0] = C::identity(),
        12 => protocol.preprocessed.clear(),
        _ => {}
    }
    let mut structure = kagemusha_protocol_structure_digest_v1(&protocol, actual_parity)
        .map_err(transcript_error)?;
    if mutation == 7 {
        structure[0] ^= 1;
    }
    let mut base = BaseCircuitBuilder::<C::ScalarExt>::new(false)
        .use_k(TEST_K)
        .use_lookup_bits(8)
        .use_instance_columns(1);
    let expected_limbs = expected_values.map(|value| base.main(0).load_witness(value));
    base.assigned_instances = vec![expected_limbs.to_vec()];
    let range = base.range_chip();
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut base, &coordinate, &scalar_integer);
    let mut jobs = PastaNativePoseidonJobsV1::new(2, TEST_ROWS).map_err(transcript_error)?;
    for pass in 0..2 {
        let loaded = if native {
            load_and_constrain_claim_protocol_native_v1(
                &loader,
                &protocol,
                actual_parity,
                structure,
                &expected_limbs,
                &mut jobs,
            )?
        } else {
            load_and_constrain_parent_protocol_v1(
                &loader,
                &protocol,
                actual_parity,
                structure,
                &expected_limbs,
            )?
        };
        for (index, point) in loaded.protocol.preprocessed.iter().enumerate() {
            assert_eq!(
                loader
                    .ecc_chip()
                    .assigned_point_source_index(&point.assigned())?,
                pass * protocol.preprocessed.len() + index,
                "repeated witness-valued preprocessing must never intern",
            );
        }
    }
    assert_eq!(
        loader.ecc_chip().witness().sources.len(),
        2 * protocol.preprocessed.len()
    );
    if native {
        assert_eq!(
            jobs.required_rows().unwrap(),
            (protocol.preprocessed.len() + 7) * 66
        );
        assert_eq!(jobs.clone().unknown().required_rows(), jobs.required_rows());
    }
    *base.pool(0) = loader.take_ctx();
    super::super::base_packing::finalize_base_params_v1(&mut base, 9).map_err(transcript_error)?;
    Ok((
        IdentityCircuit { base, jobs },
        vec![expected_values.to_vec()],
    ))
}

fn assert_protocol_identity_reference<C>(parity: KagemushaPastaParityV1)
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let mut counts = Vec::new();
    for native in [false, true] {
        let (circuit, instances) = identity_fixture::<C>(parity, native, 0).unwrap();
        counts.push(circuit.base.statistics().gate.total_advice_per_phase[0]);
        MockProver::run(TEST_K as u32, &circuit, instances)
            .expect("complete protocol-identity circuit")
            .assert_satisfied();
    }
    assert!(
        counts[1] < counts[0],
        "native protocol hashes must reduce Base cells"
    );
}

fn assert_protocol_identity_mutations<C>(parity: KagemushaPastaParityV1)
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    for native in [false, true] {
        for mutation in 1..=12 {
            let accepted = match identity_fixture::<C>(parity, native, mutation) {
                Ok((circuit, instances)) => MockProver::run(TEST_K as u32, &circuit, instances)
                    .expect("mutated protocol-identity circuit")
                    .verify()
                    .is_ok(),
                Err(_) => false,
            };
            assert!(!accepted, "native={native} protocol mutation={mutation}");
        }
    }
}

#[test]
fn claim_protocol_native_identity_matches_reference_and_preserves_fresh_sources_in_both_fields() {
    assert_protocol_identity_reference::<EqAffine>(KagemushaPastaParityV1::Eq);
    assert_protocol_identity_reference::<EpAffine>(KagemushaPastaParityV1::Ep);
}

#[test]
fn claim_protocol_native_identity_rejects_preimage_and_public_binding_mutations_in_both_fields() {
    assert_protocol_identity_mutations::<EqAffine>(KagemushaPastaParityV1::Eq);
    assert_protocol_identity_mutations::<EpAffine>(KagemushaPastaParityV1::Ep);
}
