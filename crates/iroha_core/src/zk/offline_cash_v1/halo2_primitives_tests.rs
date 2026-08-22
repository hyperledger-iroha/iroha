use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    halo2curves::{
        ff::{Field, PrimeField},
        group::{prime::PrimeCurveAffine as _, GroupEncoding as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
    },
    plonk::{
        create_proof as halo2_create_proof, keygen_pk, keygen_vk, Advice, Circuit, Column,
        ConstraintSystem, Error as PlonkError, Instance, VerifyingKey,
    },
    poly::{
        commitment::{Params as _, ParamsProver as _},
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            multiopen::ProverIPA,
        },
    },
    transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
    SerdeFormat,
};
use rand_core_06::OsRng;

use super::halo2_primitives::{
    parse_offline_cash_ep_params_v1, parse_offline_cash_eq_params_v1,
    parse_processed_verifier_key_v1,
    test_support::{
        decide_claim_for_test, derive_claim, encode_history, history_from_ep_parts,
        history_from_eq_parts, parse_ep_history, parse_eq_history, parse_params_for_k,
        verify_augmented_claim_for_test,
    },
    validate_offline_cash_history_v1, OfflineCashHalo2PrimitiveErrorV1,
};
use super::OfflineCashHalo2ParityV1;

const TEST_K: u32 = 4;

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

fn eq_fixture() -> (ParamsIPA<EqAffine>, VerifyingKey<EqAffine>, Vec<u8>, Fp) {
    let params = ParamsIPA::<EqAffine>::new(TEST_K);
    let value = Fp::from(7);
    let circuit = PublicValue { value };
    let vk = keygen_vk(&params, &circuit).expect("Eq test VK");
    let pk = keygen_pk(&params, vk.clone(), &circuit).expect("Eq test PK");
    let column = [value];
    let columns: [&[Fp]; 1] = [&column];
    let instances: [&[&[Fp]]; 1] = [&columns];
    let mut transcript = Blake2bWrite::<_, EqAffine, Challenge255<EqAffine>>::init(Vec::new());
    halo2_create_proof::<
        IPACommitmentScheme<EqAffine>,
        ProverIPA<'_, EqAffine>,
        Challenge255<EqAffine>,
        _,
        _,
        _,
    >(&params, &pk, &[circuit], &instances, OsRng, &mut transcript)
    .expect("Eq test proof");
    (params, vk, transcript.finalize(), value)
}

fn ep_fixture() -> (ParamsIPA<EpAffine>, VerifyingKey<EpAffine>, Vec<u8>, Fq) {
    let params = ParamsIPA::<EpAffine>::new(TEST_K);
    let value = Fq::from(11);
    let circuit = PublicValue { value };
    let vk = keygen_vk(&params, &circuit).expect("Ep test VK");
    let pk = keygen_pk(&params, vk.clone(), &circuit).expect("Ep test PK");
    let column = [value];
    let columns: [&[Fq]; 1] = [&column];
    let instances: [&[&[Fq]]; 1] = [&columns];
    let mut transcript = Blake2bWrite::<_, EpAffine, Challenge255<EpAffine>>::init(Vec::new());
    halo2_create_proof::<
        IPACommitmentScheme<EpAffine>,
        ProverIPA<'_, EpAffine>,
        Challenge255<EpAffine>,
        _,
        _,
        _,
    >(&params, &pk, &[circuit], &instances, OsRng, &mut transcript)
    .expect("Ep test proof");
    (params, vk, transcript.finalize(), value)
}

#[test]
fn fixed_history_codec_is_challenge_major_and_strict_for_both_parities() {
    let eq_challenges = std::array::from_fn(|index| Fp::from((index + 1) as u64));
    let eq = history_from_eq_parts(eq_challenges, EqAffine::generator()).expect("Eq history");
    let eq_bytes = encode_history(&eq);
    for (index, challenge) in eq_challenges.iter().enumerate() {
        assert_eq!(
            &eq_bytes[index * 32..(index + 1) * 32],
            challenge.to_repr().as_ref()
        );
    }
    assert_eq!(
        &eq_bytes[16 * 32..],
        EqAffine::generator().to_bytes().as_ref()
    );
    assert_eq!(parse_eq_history(&eq_bytes).expect("parse Eq"), eq);
    validate_offline_cash_history_v1(OfflineCashHalo2ParityV1::Eq, &eq_bytes).expect("validate Eq");

    let ep_challenges = std::array::from_fn(|index| Fq::from((index + 17) as u64));
    let ep = history_from_ep_parts(ep_challenges, EpAffine::generator()).expect("Ep history");
    let ep_bytes = encode_history(&ep);
    assert_eq!(parse_ep_history(&ep_bytes).expect("parse Ep"), ep);
    validate_offline_cash_history_v1(OfflineCashHalo2ParityV1::Ep, &ep_bytes).expect("validate Ep");

    assert_eq!(
        parse_eq_history(&eq_bytes[..eq_bytes.len() - 1]),
        Err(OfflineCashHalo2PrimitiveErrorV1::InvalidHistory)
    );
    let mut noncanonical_scalar = eq_bytes;
    noncanonical_scalar[..32].fill(0xff);
    assert_eq!(
        parse_eq_history(&noncanonical_scalar),
        Err(OfflineCashHalo2PrimitiveErrorV1::InvalidHistory)
    );
    let mut identity = eq_bytes;
    identity[16 * 32..].copy_from_slice(EqAffine::identity().to_bytes().as_ref());
    assert_eq!(
        parse_eq_history(&identity),
        Err(OfflineCashHalo2PrimitiveErrorV1::InvalidHistory)
    );
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

#[test]
fn augmented_current_proof_strategy_binds_and_decides_eq_history() {
    let (params, vk, proof, value) = eq_fixture();
    let column = [value];
    let columns: [&[Fp]; 1] = [&column];
    let instances: [&[&[Fp]]; 1] = [&columns];
    let accumulator = derive_claim(&params, &vk, &proof, &instances).expect("derive Eq claim");
    let mut augmented = proof;
    augmented.extend_from_slice(accumulator.g.to_bytes().as_ref());
    verify_augmented_claim_for_test(&params, &vk, &augmented, &instances, &accumulator)
        .expect("verify exact Eq augmented proof");
    decide_claim_for_test(&params, &accumulator).expect("decide Eq claim");

    let mut changed_challenge = accumulator.clone();
    changed_challenge.u_packed[0] += Fp::ONE;
    assert_eq!(
        verify_augmented_claim_for_test(&params, &vk, &augmented, &instances, &changed_challenge,),
        Err(OfflineCashHalo2PrimitiveErrorV1::HistoryBindingMismatch)
    );
    assert_eq!(
        decide_claim_for_test(&params, &changed_challenge),
        Err(OfflineCashHalo2PrimitiveErrorV1::InvalidHistoryDecision)
    );
    let wrong_column = [value + Fp::ONE];
    let wrong_columns: [&[Fp]; 1] = [&wrong_column];
    let wrong_instances: [&[&[Fp]]; 1] = [&wrong_columns];
    assert_eq!(
        verify_augmented_claim_for_test(&params, &vk, &augmented, &wrong_instances, &accumulator,),
        Err(OfflineCashHalo2PrimitiveErrorV1::InvalidProof)
    );
}

#[test]
fn augmented_current_proof_strategy_binds_and_decides_ep_history() {
    let (params, vk, proof, value) = ep_fixture();
    let column = [value];
    let columns: [&[Fq]; 1] = [&column];
    let instances: [&[&[Fq]]; 1] = [&columns];
    let accumulator = derive_claim(&params, &vk, &proof, &instances).expect("derive Ep claim");
    let mut augmented = proof;
    augmented.extend_from_slice(accumulator.g.to_bytes().as_ref());
    verify_augmented_claim_for_test(&params, &vk, &augmented, &instances, &accumulator)
        .expect("verify exact Ep augmented proof");
    decide_claim_for_test(&params, &accumulator).expect("decide Ep claim");

    let mut wrong_suffix = augmented;
    let last = wrong_suffix.len() - 1;
    wrong_suffix[last] ^= 1;
    assert_eq!(
        verify_augmented_claim_for_test(&params, &vk, &wrong_suffix, &instances, &accumulator,),
        Err(OfflineCashHalo2PrimitiveErrorV1::HistoryBindingMismatch)
    );
}
