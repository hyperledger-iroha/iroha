use halo2_proofs::{
    SerdeFormat,
    circuit::{Layouter, SimpleFloorPlanner, Value},
    halo2curves::{
        ff::{Field, PrimeField},
        group::{Curve as _, GroupEncoding as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
    },
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error as PlonkError, Instance, keygen_vk},
    poly::{
        commitment::{Params as _, ParamsProver as _},
        ipa::commitment::ParamsIPA,
    },
};
use iroha_data_model::offline::OfflineCashIpaLineageV1;

use super::halo2_primitives::{
    OfflineCashHalo2PrimitiveErrorV1, parse_offline_cash_ep_params_v1,
    parse_offline_cash_eq_params_v1, parse_processed_verifier_key_v1,
    test_support::parse_params_for_k,
};
use super::helper_recursion::{
    OfflineCashRecursiveLineageErrorV1, offline_cash_lineage_to_ep_v1,
    offline_cash_lineage_to_eq_v1,
};

const TEST_K: u32 = 4;
const FIXTURE_EQ_FOLDED_GENERATOR_V1: [u8; 32] = [
    0x00, 0x00, 0x00, 0x00, 0x21, 0xeb, 0x46, 0x8c, 0xdd, 0xa8, 0x94, 0x09, 0xfc, 0x98, 0x46, 0x22,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40,
];
const FIXTURE_EP_FOLDED_GENERATOR_V1: [u8; 32] = [
    0x00, 0x00, 0x00, 0x00, 0xed, 0x30, 0x2d, 0x99, 0x1b, 0xf9, 0x4c, 0x09, 0xfc, 0x98, 0x46, 0x22,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40,
];

#[derive(Clone)]
struct PublicValue<F: Field> {
    value: F,
}

impl<F: Field> Default for PublicValue<F> {
    fn default() -> Self {
        Self { value: F::ZERO }
    }
}

impl<F: Field> Circuit<F> for PublicValue<F> {
    type Config = (Column<Advice>, Column<Instance>);
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advice = meta.advice_column();
        let instance = meta.instance_column();
        meta.enable_equality(advice);
        meta.enable_equality(instance);
        (advice, instance)
    }

    fn synthesize(
        &self,
        (advice, instance): Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        let cell = layouter.assign_region(
            || "offline cash primitive public value",
            |mut region| {
                let cell = crate::zk::halo2_backend::assign_advice_compat(
                    &mut region,
                    || "value",
                    advice,
                    0,
                    || Value::known(self.value),
                )?;
                Ok(cell.cell())
            },
        )?;
        layouter.constrain_instance(cell, instance, 0);
        Ok(())
    }
}

fn params_bytes<C: halo2_proofs::halo2curves::CurveAffine>(params: &ParamsIPA<C>) -> Vec<u8> {
    let mut bytes = Vec::new();
    params.write(&mut bytes).expect("serialize test parameters");
    bytes
}

#[test]
fn carried_lineage_is_curve_aware_and_rejects_noncanonical_values() {
    let eq_challenges = std::array::from_fn(|index| {
        let encoded = Fp::from((index + 1) as u64).to_repr();
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(encoded.as_ref());
        bytes
    });
    let mut eq_generator = [0_u8; 32];
    eq_generator.copy_from_slice(EqAffine::generator().to_bytes().as_ref());
    assert_eq!(eq_generator, FIXTURE_EQ_FOLDED_GENERATOR_V1);
    let eq_lineage = OfflineCashIpaLineageV1::new(eq_challenges, eq_generator)
        .expect("canonical Eq lineage wire");
    let parsed_eq = offline_cash_lineage_to_eq_v1(&eq_lineage).expect("strict Eq lineage");
    assert_eq!(parsed_eq.xi.len(), 16);
    assert_eq!(parsed_eq.u, EqAffine::generator());

    let ep_challenges = std::array::from_fn(|index| {
        let encoded = Fq::from((index + 17) as u64).to_repr();
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(encoded.as_ref());
        bytes
    });
    let mut ep_generator = [0_u8; 32];
    ep_generator.copy_from_slice(EpAffine::generator().to_bytes().as_ref());
    assert_eq!(ep_generator, FIXTURE_EP_FOLDED_GENERATOR_V1);
    let ep_lineage = OfflineCashIpaLineageV1::new(ep_challenges, ep_generator)
        .expect("canonical Ep lineage wire");
    let parsed_ep = offline_cash_lineage_to_ep_v1(&ep_lineage).expect("strict Ep lineage");
    assert_eq!(parsed_ep.xi.len(), 16);
    assert_eq!(parsed_ep.u, EpAffine::generator());

    let mut noncanonical_scalar = eq_lineage;
    noncanonical_scalar.round_challenges[..32].fill(0xff);
    assert!(matches!(
        offline_cash_lineage_to_eq_v1(&noncanonical_scalar),
        Err(OfflineCashRecursiveLineageErrorV1::NonCanonicalScalar)
    ));

    assert!(matches!(
        offline_cash_lineage_to_ep_v1(&eq_lineage),
        Err(OfflineCashRecursiveLineageErrorV1::NonCanonicalOrIdentityPoint)
    ));
    let one_bit_invalid = (0..256)
        .find_map(|bit| {
            let mut mutated = eq_lineage;
            mutated.folded_generator[bit / 8] ^= 1 << (bit % 8);
            matches!(
                offline_cash_lineage_to_eq_v1(&mutated),
                Err(OfflineCashRecursiveLineageErrorV1::NonCanonicalOrIdentityPoint)
            )
            .then_some(mutated)
        })
        .expect("at least one one-bit point mutation is non-canonical");
    assert!(matches!(
        offline_cash_lineage_to_eq_v1(&one_bit_invalid),
        Err(OfflineCashRecursiveLineageErrorV1::NonCanonicalOrIdentityPoint)
    ));

    let eq_only_point = (1_u64..=1_024)
        .find_map(|scalar| {
            let point = (EqAffine::generator() * Fp::from(scalar)).to_affine();
            let mut bytes = [0_u8; 32];
            bytes.copy_from_slice(point.to_bytes().as_ref());
            Option::<EpAffine>::from(EpAffine::from_bytes(&bytes.into()))
                .is_none()
                .then_some(bytes)
        })
        .expect("an Eq point outside the Ep curve exists in the bounded search");
    let wrong_ep_curve = OfflineCashIpaLineageV1::new(ep_challenges, eq_only_point)
        .expect("field-neutral lineage accepts curve parsing in Core");
    assert!(matches!(
        offline_cash_lineage_to_ep_v1(&wrong_ep_curve),
        Err(OfflineCashRecursiveLineageErrorV1::NonCanonicalOrIdentityPoint)
    ));

    let ep_only_point = (1_u64..=1_024)
        .find_map(|scalar| {
            let point = (EpAffine::generator() * Fq::from(scalar)).to_affine();
            let mut bytes = [0_u8; 32];
            bytes.copy_from_slice(point.to_bytes().as_ref());
            Option::<EqAffine>::from(EqAffine::from_bytes(&bytes.into()))
                .is_none()
                .then_some(bytes)
        })
        .expect("an Ep point outside the Eq curve exists in the bounded search");
    let wrong_eq_curve = OfflineCashIpaLineageV1::new(eq_challenges, ep_only_point)
        .expect("field-neutral lineage accepts curve parsing in Core");
    assert!(matches!(
        offline_cash_lineage_to_eq_v1(&wrong_eq_curve),
        Err(OfflineCashRecursiveLineageErrorV1::NonCanonicalOrIdentityPoint)
    ));
}

#[test]
fn params_parser_preflights_roundtrips_and_requires_transparent_derivation() {
    let eq_params = ParamsIPA::<EqAffine>::new(TEST_K);
    let eq_bytes = params_bytes(&eq_params);
    let parsed = parse_params_for_k::<EqAffine>(&eq_bytes, TEST_K).expect("canonical Eq params");
    assert_eq!(parsed.k(), TEST_K);

    let ep_params = ParamsIPA::<EpAffine>::new(TEST_K);
    let ep_bytes = params_bytes(&ep_params);
    assert_eq!(
        parse_params_for_k::<EpAffine>(&ep_bytes, TEST_K)
            .expect("canonical Ep params")
            .k(),
        TEST_K
    );

    let mut wrong_k = eq_bytes.clone();
    wrong_k[..4].copy_from_slice(&(TEST_K + 1).to_le_bytes());
    assert_eq!(
        parse_params_for_k::<EqAffine>(&wrong_k, TEST_K).unwrap_err(),
        OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape
    );
    let mut trailing = eq_bytes.clone();
    trailing.push(0);
    assert_eq!(
        parse_params_for_k::<EqAffine>(&trailing, TEST_K).unwrap_err(),
        OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape
    );

    let mut valid_but_nontransparent = eq_bytes;
    let replacement = valid_but_nontransparent[36..68].to_vec();
    valid_but_nontransparent[4..36].copy_from_slice(&replacement);
    assert_eq!(
        parse_params_for_k::<EqAffine>(&valid_but_nontransparent, TEST_K).unwrap_err(),
        OfflineCashHalo2PrimitiveErrorV1::NonTransparentParameters
    );

    assert_eq!(
        parse_offline_cash_eq_params_v1(&[16, 0, 0, 0]).unwrap_err(),
        OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape
    );
    assert_eq!(
        parse_offline_cash_ep_params_v1(&[16, 0, 0, 0]).unwrap_err(),
        OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape
    );
}

#[test]
fn processed_vk_parser_preflights_exact_circuit_and_roundtrips_both_parities() {
    let eq_params = ParamsIPA::<EqAffine>::new(TEST_K);
    let eq_circuit = PublicValue { value: Fp::from(7) };
    let eq_bytes = keygen_vk(&eq_params, &eq_circuit)
        .expect("Eq VK")
        .to_bytes(SerdeFormat::Processed);
    parse_processed_verifier_key_v1::<EqAffine, PublicValue<Fp>>(&eq_bytes, TEST_K)
        .expect("canonical Eq processed VK");

    let ep_params = ParamsIPA::<EpAffine>::new(TEST_K);
    let ep_circuit = PublicValue {
        value: Fq::from(11),
    };
    let ep_bytes = keygen_vk(&ep_params, &ep_circuit)
        .expect("Ep VK")
        .to_bytes(SerdeFormat::Processed);
    parse_processed_verifier_key_v1::<EpAffine, PublicValue<Fq>>(&ep_bytes, TEST_K)
        .expect("canonical Ep processed VK");

    let mut wrong_version = eq_bytes.clone();
    wrong_version[0] ^= 1;
    assert_eq!(
        parse_processed_verifier_key_v1::<EqAffine, PublicValue<Fp>>(&wrong_version, TEST_K,)
            .unwrap_err(),
        OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyShape
    );
    let mut compressed = eq_bytes.clone();
    compressed[5] = 1;
    assert_eq!(
        parse_processed_verifier_key_v1::<EqAffine, PublicValue<Fp>>(&compressed, TEST_K)
            .unwrap_err(),
        OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyShape
    );
    let mut wrong_fixed_count = eq_bytes.clone();
    wrong_fixed_count[6..10].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        parse_processed_verifier_key_v1::<EqAffine, PublicValue<Fp>>(&wrong_fixed_count, TEST_K,)
            .unwrap_err(),
        OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyShape
    );
    let mut trailing = eq_bytes;
    trailing.push(0);
    assert_eq!(
        parse_processed_verifier_key_v1::<EqAffine, PublicValue<Fp>>(&trailing, TEST_K)
            .unwrap_err(),
        OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyEncoding
    );
}
