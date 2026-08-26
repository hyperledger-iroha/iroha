//! Unit tests validating the deterministic IPA and polynomial opening flow.
use super::*;
#[cfg(feature = "goldilocks_backend")]
use crate::backend::goldilocks::{self as gold, GoldilocksBackend};
use crate::{
    PolyOpenTranscriptMetadata,
    backend::{
        bn254::{self as bn254, Bn254Backend},
        pallas::{self as pallas, PallasBackend},
    },
    errors::Error,
    norito_helpers as nh,
};
use core::num::NonZeroUsize;
static PARAMS_REGISTRY_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn sample_pallas_coeffs(n: usize) -> Vec<pallas::Scalar> {
    (0..n)
        .map(|i| pallas::Scalar::from((i + 1) as u64))
        .collect()
}
#[cfg(feature = "goldilocks_backend")]
fn sample_goldilocks_coeffs(n: usize) -> Vec<gold::Scalar> {
    (0..n).map(|i| gold::Scalar::from((i + 1) as u64)).collect()
}
fn sample_bn254_coeffs(n: usize) -> Vec<bn254::Scalar> {
    (0..n)
        .map(|i| bn254::Scalar::from((i + 1) as u64))
        .collect()
}
fn sample_pallas_envelope(n: usize, label: &str) -> OpenVerifyEnvelope {
    let params = pallas::Params::new(n).unwrap();
    let coeffs = sample_pallas_coeffs(n);
    let poly = pallas::Polynomial::from_coeffs(coeffs);
    let commitment = poly.commit(&params).unwrap();
    let z = pallas::Scalar::from(5u64);
    let mut tr = Transcript::new(label);
    let (proof, t) = poly.open(&params, &mut tr, z, commitment).unwrap();
    OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, commitment),
        proof: nh::proof_to_wire(&proof),
        transcript_label: label.into(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    }
}
fn absorb_optional_test_metadata(
    transcript: &mut Transcript,
    scope: &str,
    value: Option<[u8; 32]>,
) {
    let mut payload = [0u8; 33];
    if let Some(bytes) = value {
        payload[0] = 1;
        payload[1..].copy_from_slice(&bytes);
    }
    transcript.absorb(scope, &payload);
}
fn absorb_pallas_poly_statement(
    transcript: &mut Transcript,
    params: &pallas::Params,
    z: pallas::Scalar,
    commitment: pallas::Group,
    t: pallas::Scalar,
    metadata: PolyOpenTranscriptMetadata,
) {
    transcript.absorb(
        "poly.curve_id",
        &<PallasBackend as IpaBackend>::CURVE_ID
            .as_u16()
            .to_le_bytes(),
    );
    transcript.absorb("poly.n", &(params.n() as u32).to_le_bytes());
    transcript.absorb("poly.params_fingerprint", &params.fingerprint());
    transcript.absorb("poly.z", &z.to_bytes());
    transcript.absorb("poly.t", &t.to_bytes());
    transcript.absorb("poly.p_g", &commitment.to_bytes());
    absorb_optional_test_metadata(transcript, "poly.vk_commitment", metadata.vk_commitment);
    absorb_optional_test_metadata(
        transcript,
        "poly.public_inputs_schema_hash",
        metadata.public_inputs_schema_hash,
    );
    absorb_optional_test_metadata(transcript, "poly.domain_tag", metadata.domain_tag);
}
fn sample_pallas_opening(
    n: usize,
    label: &str,
) -> (
    pallas::Params,
    pallas::Scalar,
    pallas::Group,
    pallas::Scalar,
    pallas::IpaProof,
) {
    let params = pallas::Params::new(n).unwrap();
    let poly = pallas::Polynomial::from_coeffs(sample_pallas_coeffs(n));
    let commitment = poly.commit(&params).unwrap();
    let z = pallas::Scalar::from(5u64);
    let mut transcript = Transcript::new(label);
    let (proof, t) = poly.open(&params, &mut transcript, z, commitment).unwrap();
    (params, z, commitment, t, proof)
}
fn pallas_evaluation_vector(n: usize, z: pallas::Scalar) -> Vec<pallas::Scalar> {
    let mut b = Vec::with_capacity(n);
    let mut pow = pallas::Scalar::one();
    for _ in 0..n {
        b.push(pow);
        pow = pow.mul(z);
    }
    b
}
fn naive_msm<B: IpaBackend>(bases: &[B::Group], scalars: &[B::Scalar]) -> B::Group {
    bases
        .iter()
        .zip(scalars.iter())
        .fold(B::Group::identity(), |acc, (base, scalar)| {
            acc.mul(base.pow(*scalar))
        })
}
fn forged_zero_round_envelope<B: IpaBackend>() -> OpenVerifyEnvelope {
    let one = B::Scalar::one();
    let a_final = one.add(one);
    let t = one;
    let g = B::derive_group_elem(b"G", 1, 0);
    let u = B::derive_group_elem(b"U", 1, 0);
    // For n=1 the old verifier had no challenge. Choosing
    // P = G^a * U^(a-t) made its final equality hold without knowing a
    // G-only commitment opening whose evaluation was t.
    let p_g = g.pow(a_final).mul(u.pow(a_final.sub(t)));
    OpenVerifyEnvelope {
        params: IpaParams {
            version: 1,
            curve_id: B::CURVE_ID.as_u16(),
            n: 1,
        },
        public: nh::poly_open_public::<B>(1, one, t, p_g),
        proof: IpaProofData {
            version: 1,
            l: Vec::new(),
            r: Vec::new(),
            a_final: a_final.to_bytes(),
            b_final: one.to_bytes(),
        },
        transcript_label: "forged-zero-round".into(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    }
}
#[test]
fn params_power_of_two() {
    let pallas_params = pallas::Params::new(8).expect("n=8");
    assert_eq!(pallas_params.n(), 8);
    assert_eq!(pallas_params.g().len(), 8);
    let pallas_again = pallas::Params::new(8).expect("n=8");
    assert_eq!(pallas_params.g(), pallas_again.g());
    assert_eq!(pallas_params.h(), pallas_again.h());
    #[cfg(feature = "goldilocks_backend")]
    {
        let gold_params = gold::Params::new(8).expect("n=8");
        assert_eq!(gold_params.n(), 8);
        assert_eq!(gold_params.g().len(), 8);
        let gold_again = gold::Params::new(8).expect("n=8");
        assert_eq!(gold_params.g(), gold_again.g());
        assert_eq!(gold_params.h(), gold_again.h());
    }
}
#[test]
fn polynomial_transcript_binds_the_complete_parameter_set() {
    let canonical = pallas::Params::new(8).expect("canonical parameters");
    let custom = canonical.with_rotated_generators_for_test();
    let z = pallas::Scalar::from(3_u64);
    let t = pallas::Scalar::from(5_u64);
    let commitment = canonical.g()[0];
    let mut canonical_transcript = Transcript::new("parameter-binding");
    absorb_pallas_poly_statement(
        &mut canonical_transcript,
        &canonical,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let mut custom_transcript = Transcript::new("parameter-binding");
    absorb_pallas_poly_statement(
        &mut custom_transcript,
        &custom,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    assert_ne!(
        canonical_transcript.cur_digest(),
        custom_transcript.cur_digest()
    );
}
#[test]
fn backend_msm_matches_naive_accumulation() {
    let pallas_params = pallas::Params::new(8).unwrap();
    let pallas_scalars = sample_pallas_coeffs(8);
    let pallas_msm = PallasBackend::msm(pallas_params.g(), &pallas_scalars).unwrap();
    assert_eq!(
        pallas_msm,
        naive_msm::<PallasBackend>(pallas_params.g(), &pallas_scalars)
    );
    let bn254_params = bn254::Params::new(8).unwrap();
    let bn254_scalars = sample_bn254_coeffs(8);
    let bn254_msm = Bn254Backend::msm(bn254_params.g(), &bn254_scalars).unwrap();
    assert_eq!(
        bn254_msm,
        naive_msm::<Bn254Backend>(bn254_params.g(), &bn254_scalars)
    );
}
#[test]
fn backend_msm_rejects_dimension_mismatch() {
    let params = pallas::Params::new(8).unwrap();
    let scalars = sample_pallas_coeffs(7);
    let err = PallasBackend::msm(params.g(), &scalars).unwrap_err();
    assert!(matches!(
        err,
        Error::DimensionMismatch {
            expected: 8,
            actual: 7,
        }
    ));
}
#[test]
fn params_invalid_n() {
    assert!(pallas::Params::new(0).is_err());
    assert!(matches!(pallas::Params::new(1), Err(Error::InvalidN(1))));
    assert!(pallas::Params::new(3).is_err());
    assert!(matches!(bn254::Params::new(1), Err(Error::InvalidN(1))));
    let pallas_wire = IpaParams {
        version: 1,
        curve_id: ZkCurveId::Pallas.as_u16(),
        n: 1,
    };
    assert!(matches!(
        nh::params_from_wire::<PallasBackend>(&pallas_wire),
        Err(Error::InvalidN(1))
    ));
    let bn254_wire = IpaParams {
        curve_id: ZkCurveId::Bn254.as_u16(),
        ..pallas_wire
    };
    assert!(matches!(
        nh::params_from_wire::<Bn254Backend>(&bn254_wire),
        Err(Error::InvalidN(1))
    ));
    #[cfg(feature = "goldilocks_backend")]
    {
        assert!(gold::Params::new(0).is_err());
        assert!(matches!(gold::Params::new(1), Err(Error::InvalidN(1))));
        assert!(gold::Params::new(3).is_err());
    }
}
#[test]
fn forged_zero_round_pallas_opening_is_rejected() {
    let envelope = forged_zero_round_envelope::<PallasBackend>();
    let results =
        batch::verify_open_batch_with_options(&[envelope], &batch::BatchOptions::sequential());
    assert!(matches!(results.first(), Some(Err(Error::InvalidN(1)))));
}
#[test]
fn forged_zero_round_bn254_opening_is_rejected() {
    let envelope = forged_zero_round_envelope::<Bn254Backend>();
    let results =
        batch::verify_open_batch_with_options(&[envelope], &batch::BatchOptions::sequential());
    assert!(matches!(results.first(), Some(Err(Error::InvalidN(1)))));
}
#[test]
fn poly_commit_open_verify_pallas() {
    let params = pallas::Params::new(8).unwrap();
    let coeffs = sample_pallas_coeffs(8);
    let poly = pallas::Polynomial::from_coeffs(coeffs);
    let mut tr = Transcript::new("test");
    let commitment = poly.commit(&params).unwrap();
    let z = pallas::Scalar::from(7u64);
    let (proof, t) = poly.open(&params, &mut tr, z, commitment).unwrap();
    let mut tr_v = Transcript::new("test");
    pallas::Polynomial::verify_open(&params, &mut tr_v, z, commitment, t, &proof).unwrap();
}
#[cfg(feature = "goldilocks_backend")]
#[test]
fn poly_commit_open_verify_goldilocks() {
    let params = gold::Params::new(8).unwrap();
    let coeffs = sample_goldilocks_coeffs(8);
    let poly = gold::Polynomial::from_coeffs(coeffs);
    let mut tr = Transcript::new("test-gold");
    let commitment = poly.commit(&params).unwrap();
    let z = gold::Scalar::from(5u64);
    let (proof, t) = poly.open(&params, &mut tr, z, commitment).unwrap();
    let mut tr_v = Transcript::new("test-gold");
    gold::Polynomial::verify_open(&params, &mut tr_v, z, commitment, t, &proof).unwrap();
}
#[test]
fn poly_commit_open_verify_bn254() {
    let params = bn254::Params::new(8).unwrap();
    let coeffs = sample_bn254_coeffs(8);
    let poly = bn254::Polynomial::from_coeffs(coeffs);
    let mut tr = Transcript::new("test-bn");
    let commitment = poly.commit(&params).unwrap();
    let z = bn254::Scalar::from(5u64);
    let (proof, t) = poly.open(&params, &mut tr, z, commitment).unwrap();
    let mut tr_v = Transcript::new("test-bn");
    bn254::Polynomial::verify_open(&params, &mut tr_v, z, commitment, t, &proof).unwrap();
}
#[test]
fn poly_verify_fails_on_wrong_t() {
    let params = pallas::Params::new(8).unwrap();
    let coeffs = sample_pallas_coeffs(8);
    let poly = pallas::Polynomial::from_coeffs(coeffs);
    let mut tr = Transcript::new("test");
    let commitment = poly.commit(&params).unwrap();
    let z = pallas::Scalar::from(5u64);
    let (proof, _t) = poly.open(&params, &mut tr, z, commitment).unwrap();
    let mut tr_v = Transcript::new("test");
    let t_wrong = pallas::Scalar::from(123u64);
    let err = pallas::Polynomial::verify_open(&params, &mut tr_v, z, commitment, t_wrong, &proof)
        .unwrap_err();
    assert!(matches!(err, Error::VerificationFailed));
}
#[test]
fn poly_verify_fails_on_wrong_t_bn254() {
    let params = bn254::Params::new(8).unwrap();
    let coeffs = sample_bn254_coeffs(8);
    let poly = bn254::Polynomial::from_coeffs(coeffs);
    let mut tr = Transcript::new("test-bn");
    let commitment = poly.commit(&params).unwrap();
    let z = bn254::Scalar::from(7u64);
    let (proof, _t) = poly.open(&params, &mut tr, z, commitment).unwrap();
    let mut tr_v = Transcript::new("test-bn");
    let t_wrong = bn254::Scalar::from(999u64);
    let err = bn254::Polynomial::verify_open(&params, &mut tr_v, z, commitment, t_wrong, &proof)
        .unwrap_err();
    assert!(matches!(err, Error::VerificationFailed));
}
#[test]
fn norito_roundtrip_params_and_proof_pallas() {
    let params = pallas::Params::new(8).unwrap();
    let coeffs = sample_pallas_coeffs(8);
    let poly = pallas::Polynomial::from_coeffs(coeffs);
    let commitment = poly.commit(&params).unwrap();
    let z = pallas::Scalar::from(3u64);
    let mut tr = Transcript::new("test");
    let (proof, t) = poly.open(&params, &mut tr, z, commitment).unwrap();
    let w_params = nh::params_to_wire(&params);
    let bytes_params = w_params.encode_bytes();
    let w_params2 = IpaParams::decode_bytes(&bytes_params).unwrap();
    let params2 = nh::params_from_wire::<PallasBackend>(&w_params2).unwrap();
    assert_eq!(params.n(), params2.n());
    assert_eq!(params.g(), params2.g());
    assert_eq!(params.h(), params2.h());
    let w_proof = nh::proof_to_wire(&proof);
    let bytes_proof = w_proof.encode_bytes();
    let w_proof2 = IpaProofData::decode_bytes(&bytes_proof).unwrap();
    let proof2 = nh::proof_from_wire::<PallasBackend>(&w_proof2).unwrap();
    assert_eq!(proof.l_vec, proof2.l_vec);
    assert_eq!(proof.r_vec, proof2.r_vec);
    assert_eq!(proof.a_final.to_bytes(), proof2.a_final.to_bytes());
    assert_eq!(proof.b_final.to_bytes(), proof2.b_final.to_bytes());
    let mut tr_v = Transcript::new("test");
    pallas::Polynomial::verify_open(&params2, &mut tr_v, z, commitment, t, &proof2).unwrap();
}
#[test]
fn norito_roundtrip_params_and_proof_bn254() {
    let params = bn254::Params::new(8).unwrap();
    let coeffs = sample_bn254_coeffs(8);
    let poly = bn254::Polynomial::from_coeffs(coeffs);
    let commitment = poly.commit(&params).unwrap();
    let z = bn254::Scalar::from(3u64);
    let mut tr = Transcript::new("test-bn");
    let (proof, t) = poly.open(&params, &mut tr, z, commitment).unwrap();
    let w_params = nh::params_to_wire(&params);
    let bytes_params = w_params.encode_bytes();
    let w_params2 = IpaParams::decode_bytes(&bytes_params).unwrap();
    let params2 = nh::params_from_wire::<Bn254Backend>(&w_params2).unwrap();
    assert_eq!(params.n(), params2.n());
    assert_eq!(params.g(), params2.g());
    assert_eq!(params.h(), params2.h());
    let w_proof = nh::proof_to_wire(&proof);
    let bytes_proof = w_proof.encode_bytes();
    let w_proof2 = IpaProofData::decode_bytes(&bytes_proof).unwrap();
    let proof2 = nh::proof_from_wire::<Bn254Backend>(&w_proof2).unwrap();
    assert_eq!(proof.l_vec, proof2.l_vec);
    assert_eq!(proof.r_vec, proof2.r_vec);
    assert_eq!(proof.a_final.to_bytes(), proof2.a_final.to_bytes());
    assert_eq!(proof.b_final.to_bytes(), proof2.b_final.to_bytes());
    let mut tr_v = Transcript::new("test-bn");
    bn254::Polynomial::verify_open(&params2, &mut tr_v, z, commitment, t, &proof2).unwrap();
}
fn assert_checked_bare_decoder<T>(
    bytes: &[u8],
    decode: impl Fn(&[u8]) -> Result<T, norito::Error>,
) {
    let empty_error = match decode(&[]) {
        Ok(_) => panic!("empty payload must not decode"),
        Err(error) => error,
    };
    assert!(matches!(empty_error, norito::Error::LengthMismatch));
    let truncated = bytes
        .get(..bytes.len().saturating_sub(1))
        .expect("non-empty encoded payload");
    assert!(
        decode(truncated).is_err(),
        "truncated payload must not decode"
    );
    let mut trailing = bytes.to_vec();
    trailing.push(0xA5);
    let trailing_error = match decode(&trailing) {
        Ok(_) => panic!("trailing payload must not decode"),
        Err(error) => error,
    };
    assert!(matches!(
        trailing_error,
        norito::Error::LengthMismatch | norito::Error::NonCanonicalEncoding
    ));
    let align = norito::core::archived_payload_align::<T>();
    if align > 1 {
        let mut storage = vec![0_u8; bytes.len() + align];
        let base = storage.as_ptr() as usize;
        let offset = (0..align)
            .find(|offset| !(base + offset).is_multiple_of(align))
            .expect("an alignment greater than one has a misaligned offset");
        storage[offset..offset + bytes.len()].copy_from_slice(bytes);
        decode(&storage[offset..offset + bytes.len()])
            .expect("misaligned payload should be realigned and decoded");
    }
}
#[test]
fn standalone_norito_decoders_are_bounded_exact_and_alignment_safe() {
    let params = IpaParams {
        version: 1,
        curve_id: ZkCurveId::Pallas.as_u16(),
        n: 1,
    };
    assert_checked_bare_decoder(&params.encode_bytes(), IpaParams::decode_bytes);
    let proof = IpaProofData {
        version: 1,
        l: vec![[4; 32]],
        r: vec![[5; 32]],
        a_final: [6; 32],
        b_final: [7; 32],
    };
    assert_checked_bare_decoder(&proof.encode_bytes(), IpaProofData::decode_bytes);
    let public = PolyOpenPublic {
        version: 1,
        curve_id: ZkCurveId::Pallas.as_u16(),
        n: 1,
        z: [8; 32],
        t: [9; 32],
        p_g: [10; 32],
    };
    assert_checked_bare_decoder(&public.encode_bytes(), PolyOpenPublic::decode_bytes);
}
#[test]
fn standalone_params_wire_never_carries_generator_material() {
    #[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
    #[derive(norito::derive::NoritoSerialize)]
    struct RetiredInlineParams {
        version: u16,
        curve_id: u16,
        n: u32,
        g: Vec<[u8; 32]>,
        h: Vec<[u8; 32]>,
        u: [u8; 32],
    }
    let small = IpaParams {
        version: 1,
        curve_id: ZkCurveId::Pallas.as_u16(),
        n: 1,
    };
    let large = IpaParams {
        n: 1 << OpenVerifyLimits::DEFAULT_MAX_K,
        ..small
    };
    assert_eq!(small.encode_bytes().len(), large.encode_bytes().len());
    let retired = RetiredInlineParams {
        version: 1,
        curve_id: ZkCurveId::Pallas.as_u16(),
        n: 1,
        g: vec![[1; 32]],
        h: vec![[2; 32]],
        u: [3; 32],
    };
    let mut retired_bytes = Vec::new();
    norito::core::serialize_to_buffer(&retired, &mut retired_bytes)
        .expect("retired inline parameter fixture must encode");
    IpaParams::decode_bytes(&retired_bytes)
        .expect_err("the canonical selector wire must reject inline generators");
}
#[cfg(feature = "goldilocks_backend")]
#[test]
fn norito_roundtrip_params_and_proof_goldilocks() {
    let params = gold::Params::new(8).unwrap();
    let coeffs = sample_goldilocks_coeffs(8);
    let poly = gold::Polynomial::from_coeffs(coeffs);
    let commitment = poly.commit(&params).unwrap();
    let z = gold::Scalar::from(4u64);
    let mut tr = Transcript::new("test-gold");
    let (proof, t) = poly.open(&params, &mut tr, z, commitment).unwrap();
    let w_params = nh::params_to_wire(&params);
    let bytes_params = w_params.encode_bytes();
    let w_params2 = IpaParams::decode_bytes(&bytes_params).unwrap();
    let params2 = nh::params_from_wire::<GoldilocksBackend>(&w_params2).unwrap();
    assert_eq!(params.n(), params2.n());
    assert_eq!(params.g(), params2.g());
    assert_eq!(params.h(), params2.h());
    let w_proof = nh::proof_to_wire(&proof);
    let bytes_proof = w_proof.encode_bytes();
    let w_proof2 = IpaProofData::decode_bytes(&bytes_proof).unwrap();
    let proof2 = nh::proof_from_wire::<GoldilocksBackend>(&w_proof2).unwrap();
    assert_eq!(proof.l_vec, proof2.l_vec);
    assert_eq!(proof.r_vec, proof2.r_vec);
    assert_eq!(proof.a_final.to_bytes(), proof2.a_final.to_bytes());
    assert_eq!(proof.b_final.to_bytes(), proof2.b_final.to_bytes());
    let mut tr_v = Transcript::new("test-gold");
    gold::Polynomial::verify_open(&params2, &mut tr_v, z, commitment, t, &proof2).unwrap();
    let envelope = OpenVerifyEnvelope {
        params: w_params,
        public: nh::poly_open_public::<GoldilocksBackend>(params.n(), z, t, commitment),
        proof: w_proof,
        transcript_label: "test-gold".into(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let results = crate::batch::verify_open_batch(std::slice::from_ref(&envelope));
    assert!(matches!(
        results[0],
        Err(Error::UnsupportedBackend {
            backend: ZkCurveId::Goldilocks
        })
    ));
}
#[test]
fn params_registry_keys_include_backend_curve() {
    let _guard = PARAMS_REGISTRY_TEST_LOCK.lock().unwrap();
    crate::params::clear_params_registry_for_tests();
    let pallas_wire = nh::params_to_wire(&pallas::Params::new(8).unwrap());
    let bn254_wire = nh::params_to_wire(&bn254::Params::new(8).unwrap());
    nh::params_from_wire::<PallasBackend>(&pallas_wire).unwrap();
    nh::params_from_wire::<Bn254Backend>(&bn254_wire).unwrap();
    assert!(crate::params::params_registry_contains_for_tests::<
        PallasBackend,
    >(8));
    assert!(crate::params::params_registry_contains_for_tests::<
        Bn254Backend,
    >(8));
    crate::params::clear_params_registry_for_tests();
}
#[test]
fn params_registry_reuses_canonical_wire_params() {
    let _guard = PARAMS_REGISTRY_TEST_LOCK.lock().unwrap();
    crate::params::clear_params_registry_for_tests();
    let wire = nh::params_to_wire(&pallas::Params::new(8).unwrap());
    let registered = nh::params_from_wire::<PallasBackend>(&wire).unwrap();
    let decoded = nh::params_from_wire::<PallasBackend>(&wire).unwrap();
    let other_wire = nh::params_to_wire(&pallas::Params::new(16).unwrap());
    let other = nh::params_from_wire::<PallasBackend>(&other_wire).unwrap();
    assert!(std::sync::Arc::ptr_eq(&registered, &decoded));
    assert!(!std::sync::Arc::ptr_eq(&registered, &other));
    assert!(crate::params::params_registry_contains_for_tests::<
        PallasBackend,
    >(16));
    crate::params::clear_params_registry_for_tests();
}
#[cfg(feature = "goldilocks_backend")]
#[test]
fn batch_verify_two_envelopes_mixed() {
    let params_p = pallas::Params::new(8).unwrap();
    let coeffs_p = sample_pallas_coeffs(8);
    let poly_p = pallas::Polynomial::from_coeffs(coeffs_p);
    let commitment_p = poly_p.commit(&params_p).unwrap();
    let z_p = pallas::Scalar::from(2u64);
    let mut tr_p = Transcript::new("batch");
    let (proof_p, t_p) = poly_p
        .open(&params_p, &mut tr_p, z_p, commitment_p)
        .unwrap();
    let params_g = gold::Params::new(8).unwrap();
    let coeffs_g = sample_goldilocks_coeffs(8);
    let poly_g = gold::Polynomial::from_coeffs(coeffs_g);
    let commitment_g = poly_g.commit(&params_g).unwrap();
    let z_g = gold::Scalar::from(3u64);
    let mut tr_g = Transcript::new("batch");
    let (proof_g, _t_g) = poly_g
        .open(&params_g, &mut tr_g, z_g, commitment_g)
        .unwrap();
    let env_ok = OpenVerifyEnvelope {
        params: nh::params_to_wire(&params_p),
        public: nh::poly_open_public::<PallasBackend>(params_p.n(), z_p, t_p, commitment_p),
        proof: nh::proof_to_wire(&proof_p),
        transcript_label: "batch".into(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let env_bad = OpenVerifyEnvelope {
        params: nh::params_to_wire(&params_g),
        public: nh::poly_open_public::<GoldilocksBackend>(
            params_g.n(),
            z_g,
            gold::Scalar::from(111u64),
            commitment_g,
        ),
        proof: nh::proof_to_wire(&proof_g),
        transcript_label: "batch".into(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let results = crate::batch::verify_open_batch(&[env_ok.clone(), env_bad.clone()]);
    assert!(matches!(results[0], Ok(true)));
    assert!(matches!(
        results[1],
        Err(Error::UnsupportedBackend {
            backend: ZkCurveId::Goldilocks
        })
    ));
    let seq_results = crate::batch::verify_open_batch_with_options(
        &[env_ok.clone(), env_bad.clone()],
        &crate::batch::BatchOptions::sequential(),
    );
    assert_eq!(
        results[0].as_ref().unwrap(),
        seq_results[0].as_ref().unwrap()
    );
    assert!(matches!(
        seq_results[1],
        Err(Error::UnsupportedBackend {
            backend: ZkCurveId::Goldilocks
        })
    ));
    let limited_results = crate::batch::verify_open_batch_with_options(
        &[env_ok, env_bad],
        &crate::batch::BatchOptions::limited(NonZeroUsize::new(1).unwrap()),
    );
    assert_eq!(
        seq_results[0].as_ref().unwrap(),
        limited_results[0].as_ref().unwrap()
    );
    assert!(matches!(
        limited_results[1],
        Err(Error::UnsupportedBackend {
            backend: ZkCurveId::Goldilocks
        })
    ));
}
#[test]
fn batch_verify_pallas_and_bn254() {
    let params_p = pallas::Params::new(8).unwrap();
    let coeffs_p = sample_pallas_coeffs(8);
    let poly_p = pallas::Polynomial::from_coeffs(coeffs_p);
    let commitment_p = poly_p.commit(&params_p).unwrap();
    let z_p = pallas::Scalar::from(2u64);
    let mut tr_p = Transcript::new("batch-mixed");
    let (proof_p, t_p) = poly_p
        .open(&params_p, &mut tr_p, z_p, commitment_p)
        .unwrap();
    let params_b = bn254::Params::new(8).unwrap();
    let coeffs_b = sample_bn254_coeffs(8);
    let poly_b = bn254::Polynomial::from_coeffs(coeffs_b);
    let commitment_b = poly_b.commit(&params_b).unwrap();
    let z_b = bn254::Scalar::from(3u64);
    let mut tr_b = Transcript::new("batch-mixed");
    let (proof_b, t_b) = poly_b
        .open(&params_b, &mut tr_b, z_b, commitment_b)
        .unwrap();
    let env_p = OpenVerifyEnvelope {
        params: nh::params_to_wire(&params_p),
        public: nh::poly_open_public::<PallasBackend>(params_p.n(), z_p, t_p, commitment_p),
        proof: nh::proof_to_wire(&proof_p),
        transcript_label: "batch-mixed".into(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let env_b = OpenVerifyEnvelope {
        params: nh::params_to_wire(&params_b),
        public: nh::poly_open_public::<Bn254Backend>(params_b.n(), z_b, t_b, commitment_b),
        proof: nh::proof_to_wire(&proof_b),
        transcript_label: "batch-mixed".into(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let results = crate::batch::verify_open_batch(&[env_p.clone(), env_b.clone()]);
    assert!(matches!(results[0], Ok(true)));
    assert!(matches!(results[1], Ok(true)));
    let auto_results = crate::batch::verify_open_batch_with_options(
        &[env_p.clone(), env_b.clone()],
        &crate::batch::BatchOptions::auto(),
    );
    for (lhs, rhs) in results.iter().zip(auto_results.iter()) {
        assert_eq!(lhs.as_ref().unwrap(), rhs.as_ref().unwrap());
    }
    let limited_results = crate::batch::verify_open_batch_with_options(
        &[env_p, env_b],
        &crate::batch::BatchOptions::limited(NonZeroUsize::new(2).unwrap()),
    );
    for (lhs, rhs) in auto_results.iter().zip(limited_results.iter()) {
        assert_eq!(lhs.as_ref().unwrap(), rhs.as_ref().unwrap());
    }
}
#[test]
fn decode_envelope_exposes_components() {
    let params = pallas::Params::new(8).unwrap();
    let coeffs = sample_pallas_coeffs(8);
    let poly = pallas::Polynomial::from_coeffs(coeffs);
    let commitment = poly.commit(&params).unwrap();
    let z = pallas::Scalar::from(9u64);
    let mut transcript = Transcript::new("decode-test");
    let (proof, t) = poly.open(&params, &mut transcript, z, commitment).unwrap();
    let envelope = OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, commitment),
        proof: nh::proof_to_wire(&proof),
        transcript_label: "decode-test".into(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let decoded = nh::decode_envelope(&envelope).expect("envelope should decode");
    match decoded {
        nh::DecodedEnvelope::Pallas {
            params: decoded_params,
            proof: decoded_proof,
            z: decoded_z,
            t: decoded_t,
            p_g,
        } => {
            assert_eq!(decoded_params.n(), params.n());
            assert_eq!(decoded_z, z);
            assert_eq!(decoded_t, t);
            assert_eq!(p_g, commitment);
            assert_eq!(decoded_proof.l_vec.len(), proof.l_vec.len());
        }
        other => panic!("expected Pallas variant, got {other:?}"),
    }
}
#[test]
fn decode_envelope_reports_typed_wire_errors() {
    let envelope = sample_pallas_envelope(8, "decode-errors");
    let mut bad = envelope.clone();
    bad.public.version = 2;
    assert!(matches!(
        nh::decode_envelope(&bad),
        Err(Error::UnsupportedVersion {
            component: "PolyOpenPublic",
            version: 2
        })
    ));
    let mut bad = envelope.clone();
    bad.proof.version = 2;
    assert!(matches!(
        nh::decode_envelope(&bad),
        Err(Error::UnsupportedVersion {
            component: "IpaProofData",
            version: 2
        })
    ));
    let mut bad = envelope.clone();
    bad.params.version = 2;
    assert!(matches!(
        nh::decode_envelope(&bad),
        Err(Error::UnsupportedVersion {
            component: "IpaParams",
            version: 2
        })
    ));
    let mut bad = envelope.clone();
    bad.public.curve_id = ZkCurveId::Bn254.as_u16();
    assert!(matches!(
        nh::decode_envelope(&bad),
        Err(Error::CurveMismatch {
            expected: ZkCurveId::Pallas,
            actual: ZkCurveId::Bn254
        })
    ));
    let mut bad = envelope.clone();
    bad.public.n = 16;
    assert!(matches!(
        nh::decode_envelope(&bad),
        Err(Error::DimensionMismatch {
            expected: 8,
            actual: 16
        })
    ));
    let mut bad = envelope.clone();
    bad.proof.r.pop();
    assert!(matches!(
        nh::decode_envelope(&bad),
        Err(Error::InvalidProofShape {
            reason: "L/R round count",
            ..
        })
    ));
    let mut bad = envelope;
    bad.params.n = 0;
    bad.public.n = 0;
    assert!(matches!(nh::decode_envelope(&bad), Err(Error::InvalidN(0))));
}
#[test]
fn decode_envelope_reports_invalid_public_group_encoding() {
    let mut envelope = sample_pallas_envelope(8, "invalid-public-group");
    envelope.public.p_g = [0xff; 32];
    assert!(matches!(
        nh::decode_envelope(&envelope),
        Err(Error::InvalidEncoding)
    ));
}
#[cfg(not(feature = "goldilocks_backend"))]
#[test]
fn decode_envelope_reports_uncompiled_backend() {
    let mut envelope = sample_pallas_envelope(8, "unsupported-backend");
    envelope.params.curve_id = ZkCurveId::Goldilocks.as_u16();
    envelope.public.curve_id = ZkCurveId::Goldilocks.as_u16();
    assert!(matches!(
        nh::decode_envelope(&envelope),
        Err(Error::UnsupportedBackend {
            backend: ZkCurveId::Goldilocks
        })
    ));
}
#[test]
fn decode_envelope_reports_unknown_backend_id() {
    let mut envelope = sample_pallas_envelope(8, "unknown-backend");
    envelope.params.curve_id = u16::MAX;
    envelope.public.curve_id = u16::MAX;
    assert!(matches!(
        nh::decode_envelope(&envelope),
        Err(Error::UnsupportedBackend {
            backend: ZkCurveId::Unknown
        })
    ));
}
#[test]
fn decode_envelope_accepts_exact_resource_limits() {
    let envelope = sample_pallas_envelope(8, "four");
    let decoded = nh::decode_envelope_with_limits(&envelope, OpenVerifyLimits::new(3, 4)).unwrap();
    assert!(matches!(decoded, nh::DecodedEnvelope::Pallas { .. }));
}
#[test]
fn default_open_verify_limits_are_finite_v1_bounds() {
    assert_eq!(
        OpenVerifyLimits::default(),
        OpenVerifyLimits::new(
            OpenVerifyLimits::DEFAULT_MAX_K,
            OpenVerifyLimits::DEFAULT_MAX_TRANSCRIPT_LABEL_LEN,
        )
    );
    assert_ne!(OpenVerifyLimits::DEFAULT_MAX_K, u32::MAX);
    assert_ne!(
        OpenVerifyLimits::DEFAULT_MAX_TRANSCRIPT_LABEL_LEN,
        usize::MAX
    );
}
#[test]
fn decode_envelope_accepts_large_max_k_limit() {
    let envelope = sample_pallas_envelope(8, "large-max-k");
    let decoded = nh::decode_envelope_with_limits(
        &envelope,
        OpenVerifyLimits::new(usize::BITS, "large-max-k".len()),
    )
    .unwrap();
    assert!(matches!(decoded, nh::DecodedEnvelope::Pallas { .. }));
}
#[test]
fn decode_envelope_limits_reject_oversized_proof_vectors_before_dispatch() {
    let limits = OpenVerifyLimits::new(2, usize::MAX);
    let mut oversized_l = sample_pallas_envelope(4, "limit-oversized-l");
    oversized_l.proof.l.push(oversized_l.proof.l[0]);
    assert!(matches!(
        nh::decode_envelope_with_limits(&oversized_l, limits),
        Err(Error::EnvelopeLimitExceeded {
            limit: "proof_l_rounds",
            max: 2,
            actual: 3
        })
    ));
    let mut oversized_r = sample_pallas_envelope(4, "limit-oversized-r");
    oversized_r.proof.r.push(oversized_r.proof.r[0]);
    assert!(matches!(
        nh::decode_envelope_with_limits(&oversized_r, limits),
        Err(Error::EnvelopeLimitExceeded {
            limit: "proof_r_rounds",
            max: 2,
            actual: 3
        })
    ));
}
#[test]
fn params_from_wire_reports_version_and_curve_mismatch() {
    let params = pallas::Params::new(8).unwrap();
    let wire = nh::params_to_wire(&params);
    let mut bad = wire;
    bad.version = 2;
    assert!(matches!(
        nh::params_from_wire::<PallasBackend>(&bad),
        Err(Error::UnsupportedVersion {
            component: "IpaParams",
            version: 2
        })
    ));
    let mut bad = wire;
    bad.curve_id = ZkCurveId::Bn254.as_u16();
    assert!(matches!(
        nh::params_from_wire::<PallasBackend>(&bad),
        Err(Error::CurveMismatch {
            expected: ZkCurveId::Pallas,
            actual: ZkCurveId::Bn254
        })
    ));
    let mut bad = nh::params_to_wire(&params);
    bad.curve_id = u16::MAX;
    assert!(matches!(
        nh::params_from_wire::<PallasBackend>(&bad),
        Err(Error::CurveMismatch {
            expected: ZkCurveId::Pallas,
            actual: ZkCurveId::Unknown
        })
    ));
}
#[test]
fn proof_from_wire_reports_version_and_round_mismatch() {
    let envelope = sample_pallas_envelope(8, "proof-wire-errors");
    let mut bad = envelope.proof.clone();
    bad.version = 2;
    assert!(matches!(
        nh::proof_from_wire::<PallasBackend>(&bad),
        Err(Error::UnsupportedVersion {
            component: "IpaProofData",
            version: 2
        })
    ));
    let mut bad = envelope.proof;
    bad.l.pop();
    assert!(matches!(
        nh::proof_from_wire::<PallasBackend>(&bad),
        Err(Error::InvalidProofShape {
            reason: "L/R round count",
            ..
        })
    ));
}
#[test]
fn proof_from_wire_reports_invalid_scalar_encoding() {
    let envelope = sample_pallas_envelope(8, "proof-invalid-scalar");
    let mut bad = envelope.proof;
    bad.a_final = [0xff; 32];
    assert!(matches!(
        nh::proof_from_wire::<PallasBackend>(&bad),
        Err(Error::InvalidEncoding)
    ));
}
#[test]
fn proof_from_wire_reports_invalid_group_encoding() {
    let envelope = sample_pallas_envelope(8, "proof-invalid-group");
    let mut bad = envelope.proof;
    bad.l[0] = [0xff; 32];
    assert!(matches!(
        nh::proof_from_wire::<PallasBackend>(&bad),
        Err(Error::InvalidEncoding)
    ));
}
#[test]
fn batch_verify_empty_inputs_return_empty_for_all_options() {
    let limits = OpenVerifyLimits::new(0, 0);
    assert!(crate::batch::verify_open_batch(&[]).is_empty());
    assert!(
        crate::batch::verify_open_batch_with_options(
            &[],
            &crate::batch::BatchOptions::sequential()
        )
        .is_empty()
    );
    assert!(
        crate::batch::verify_open_batch_with_limits(
            &[],
            &crate::batch::BatchOptions::limited(NonZeroUsize::new(2).unwrap()),
            limits,
        )
        .is_empty()
    );
}
#[test]
fn batch_verify_enforces_open_verify_limits_before_param_registration() {
    let envelope = sample_pallas_envelope(8, "limit-test");
    let results = crate::batch::verify_open_batch_with_limits(
        std::slice::from_ref(&envelope),
        &crate::batch::BatchOptions::sequential(),
        OpenVerifyLimits::new(2, 64),
    );
    assert!(matches!(
        results[0],
        Err(Error::EnvelopeLimitExceeded { limit: "max_k", .. })
    ));
    let results = crate::batch::verify_open_batch_with_limits(
        std::slice::from_ref(&envelope),
        &crate::batch::BatchOptions::sequential(),
        OpenVerifyLimits::new(3, 4),
    );
    assert!(matches!(
        results[0],
        Err(Error::EnvelopeLimitExceeded {
            limit: "transcript_label_len",
            ..
        })
    ));
}
#[test]
fn verifier_rejects_invalid_proof_round_shape() {
    let params = pallas::Params::new(8).unwrap();
    let coeffs = sample_pallas_coeffs(8);
    let poly = pallas::Polynomial::from_coeffs(coeffs);
    let commitment = poly.commit(&params).unwrap();
    let z = pallas::Scalar::from(5u64);
    let mut tr = Transcript::new("shape-test");
    let (proof, t) = poly.open(&params, &mut tr, z, commitment).unwrap();
    let mut missing_round = proof.clone();
    missing_round.l_vec.pop();
    missing_round.r_vec.pop();
    let mut tr_v = Transcript::new("shape-test");
    let err = pallas::Polynomial::verify_open(&params, &mut tr_v, z, commitment, t, &missing_round)
        .unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidProofShape {
            reason: "round count",
            expected: 3,
            actual: 2
        }
    ));
    let mut mismatched_rounds = proof;
    mismatched_rounds.r_vec.pop();
    let mut tr_v = Transcript::new("shape-test");
    let err =
        pallas::Polynomial::verify_open(&params, &mut tr_v, z, commitment, t, &mismatched_rounds)
            .unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidProofShape {
            reason: "L/R round count",
            ..
        }
    ));
}
#[test]
fn transcript_challenges_advance_running_state() {
    let mut transcript = Transcript::new("running-state");
    let initial = transcript.cur_digest();
    let first = transcript.challenge_scalar::<pallas::Scalar>("x");
    let after_first = transcript.cur_digest();
    let second = transcript.challenge_scalar::<pallas::Scalar>("x");
    let after_second = transcript.cur_digest();
    assert_ne!(initial, after_first);
    assert_ne!(after_first, after_second);
    assert_ne!(first.to_bytes(), second.to_bytes());
    let mut replay = Transcript::new("running-state");
    assert_eq!(
        first.to_bytes(),
        replay.challenge_scalar::<pallas::Scalar>("x").to_bytes()
    );
    assert_eq!(
        second.to_bytes(),
        replay.challenge_scalar::<pallas::Scalar>("x").to_bytes()
    );
}
#[test]
fn transcript_absorb_uses_scope_and_length_boundaries() {
    let mut scoped = Transcript::new("absorb-boundary");
    scoped.absorb("ab", b"c");
    let scoped_challenge = scoped.challenge_scalar::<pallas::Scalar>("x");
    let mut differently_scoped = Transcript::new("absorb-boundary");
    differently_scoped.absorb("a", b"bc");
    let differently_scoped_challenge = differently_scoped.challenge_scalar::<pallas::Scalar>("x");
    let mut differently_labeled = Transcript::new("absorb-boundary-other");
    differently_labeled.absorb("ab", b"c");
    let differently_labeled_challenge = differently_labeled.challenge_scalar::<pallas::Scalar>("x");
    assert_ne!(
        scoped_challenge.to_bytes(),
        differently_scoped_challenge.to_bytes()
    );
    assert_ne!(
        scoped_challenge.to_bytes(),
        differently_labeled_challenge.to_bytes()
    );
}
#[test]
fn ipa_round_challenge_projection_matches_verifier_transcript() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "round-projection");
    let mut projected = Transcript::new("round-projection");
    absorb_pallas_poly_statement(
        &mut projected,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let rounds =
        derive_ipa_verifier_round_challenges::<PallasBackend>(params.n(), &mut projected, &proof)
            .expect("round challenges derive");
    assert_eq!(rounds.len(), 3);
    for (index, round) in rounds.iter().enumerate() {
        assert_eq!(round.round_index, index);
        assert_ne!(round.state_before_round, round.state_after_round_absorb);
        assert_ne!(round.state_after_round_absorb, round.state_after_challenge);
        assert_eq!(
            round.challenge.mul(round.challenge_inverse).to_bytes(),
            pallas::Scalar::one().to_bytes()
        );
        if index > 0 {
            assert_eq!(
                rounds[index - 1].state_after_challenge,
                round.state_before_round
            );
        }
    }
    let mut verified = Transcript::new("round-projection");
    pallas::Polynomial::verify_open(&params, &mut verified, z, commitment, t, &proof)
        .expect("native verifier accepts proof");
    assert_eq!(projected.cur_digest(), verified.cur_digest());
}
#[test]
fn ipa_round_challenge_projection_binds_label_and_statement_prefix() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "round-binding");
    let mut good = Transcript::new("round-binding");
    absorb_pallas_poly_statement(
        &mut good,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let good_rounds =
        derive_ipa_verifier_round_challenges::<PallasBackend>(params.n(), &mut good, &proof)
            .expect("good round projection");
    let mut wrong_label = Transcript::new("round-binding-other");
    absorb_pallas_poly_statement(
        &mut wrong_label,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let wrong_label_rounds =
        derive_ipa_verifier_round_challenges::<PallasBackend>(params.n(), &mut wrong_label, &proof)
            .expect("wrong-label round projection");
    assert_ne!(
        good_rounds[0].challenge.to_bytes(),
        wrong_label_rounds[0].challenge.to_bytes()
    );
    let mut missing_statement = Transcript::new("round-binding");
    let missing_statement_rounds = derive_ipa_verifier_round_challenges::<PallasBackend>(
        params.n(),
        &mut missing_statement,
        &proof,
    )
    .expect("missing-statement round projection still derives a different transcript");
    assert_ne!(
        good_rounds[0].challenge.to_bytes(),
        missing_statement_rounds[0].challenge.to_bytes()
    );
}
#[test]
fn ipa_round_challenge_projection_binds_round_bytes_and_order() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "round-order");
    let mut good = Transcript::new("round-order");
    absorb_pallas_poly_statement(
        &mut good,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let good_rounds =
        derive_ipa_verifier_round_challenges::<PallasBackend>(params.n(), &mut good, &proof)
            .expect("good round projection");
    let mut tampered = proof.clone();
    tampered.l_vec[0] = tampered.l_vec[0].mul(params.u());
    let mut tampered_transcript = Transcript::new("round-order");
    absorb_pallas_poly_statement(
        &mut tampered_transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let tampered_rounds = derive_ipa_verifier_round_challenges::<PallasBackend>(
        params.n(),
        &mut tampered_transcript,
        &tampered,
    )
    .expect("tampered round projection");
    assert_ne!(
        good_rounds[0].challenge.to_bytes(),
        tampered_rounds[0].challenge.to_bytes()
    );
    let mut reordered = proof.clone();
    reordered.l_vec.swap(0, 1);
    reordered.r_vec.swap(0, 1);
    let mut reordered_transcript = Transcript::new("round-order");
    absorb_pallas_poly_statement(
        &mut reordered_transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let reordered_rounds = derive_ipa_verifier_round_challenges::<PallasBackend>(
        params.n(),
        &mut reordered_transcript,
        &reordered,
    )
    .expect("reordered round projection");
    assert_ne!(
        good_rounds[0].challenge.to_bytes(),
        reordered_rounds[0].challenge.to_bytes()
    );
}
#[test]
fn ipa_round_challenge_projection_rejects_bad_shape() {
    let (params, _z, _commitment, _t, proof) = sample_pallas_opening(8, "round-shape");
    let mut invalid_n = Transcript::new("round-shape");
    assert!(matches!(
        derive_ipa_verifier_round_challenges::<PallasBackend>(0, &mut invalid_n, &proof),
        Err(Error::InvalidN(0))
    ));
    let mut zero_round_n = Transcript::new("round-shape");
    assert!(matches!(
        derive_ipa_verifier_round_challenges::<PallasBackend>(1, &mut zero_round_n, &proof),
        Err(Error::InvalidN(1))
    ));
    let mut missing_round = proof.clone();
    missing_round.r_vec.pop();
    let mut missing_round_transcript = Transcript::new("round-shape");
    assert!(matches!(
        derive_ipa_verifier_round_challenges::<PallasBackend>(
            params.n(),
            &mut missing_round_transcript,
            &missing_round,
        ),
        Err(Error::InvalidProofShape {
            reason: "L/R round count",
            ..
        })
    ));
    let mut short = proof;
    short.l_vec.pop();
    short.r_vec.pop();
    let mut short_transcript = Transcript::new("round-shape");
    assert!(matches!(
        derive_ipa_verifier_round_challenges::<PallasBackend>(
            params.n(),
            &mut short_transcript,
            &short,
        ),
        Err(Error::InvalidProofShape {
            reason: "round count",
            expected: 3,
            actual: 2,
        })
    ));
}
#[test]
fn ipa_transcript_projection_records_round_bytes_and_state_boundaries() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "transcript-projection");
    let mut transcript = Transcript::new("transcript-projection");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let state_before_ipa_n = transcript.cur_digest();
    let projection = derive_ipa_verifier_transcript_projection::<PallasBackend>(
        params.n(),
        &mut transcript,
        &proof,
    )
    .expect("transcript projection derives");
    assert_eq!(projection.n, params.n());
    assert_eq!(projection.rounds.len(), 3);
    assert_eq!(projection.state_before_ipa_n, state_before_ipa_n);
    assert_ne!(projection.state_before_ipa_n, projection.state_after_ipa_n);
    assert_eq!(
        projection.state_after_ipa_n,
        projection.rounds[0].state_before_round
    );
    assert_eq!(
        projection.final_state,
        projection
            .rounds
            .last()
            .expect("last transcript round")
            .state_after_challenge
    );
    for (index, round) in projection.rounds.iter().enumerate() {
        assert_eq!(round.round_index, index);
        assert_eq!(round.l_bytes, proof.l_vec[index].to_bytes());
        assert_eq!(round.r_bytes, proof.r_vec[index].to_bytes());
        assert_ne!(round.round_bytes_digest, [0u8; 32]);
        if index > 0 {
            assert_ne!(
                projection.rounds[index - 1].round_bytes_digest,
                round.round_bytes_digest
            );
        }
    }
    let mut validation = Transcript::new("transcript-projection");
    absorb_pallas_poly_statement(
        &mut validation,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    validate_ipa_verifier_transcript_projection::<PallasBackend>(
        params.n(),
        &mut validation,
        &proof,
        &projection,
    )
    .expect("transcript projection validates");
}
#[test]
fn ipa_transcript_projection_validation_rejects_round_byte_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "transcript-l-bytes");
    let mut transcript = Transcript::new("transcript-l-bytes");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let mut projection = derive_ipa_verifier_transcript_projection::<PallasBackend>(
        params.n(),
        &mut transcript,
        &proof,
    )
    .expect("transcript projection derives");
    projection.rounds[0].l_bytes[0] ^= 0x01;
    let mut validation = Transcript::new("transcript-l-bytes");
    absorb_pallas_poly_statement(
        &mut validation,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    assert!(matches!(
        validate_ipa_verifier_transcript_projection::<PallasBackend>(
            params.n(),
            &mut validation,
            &proof,
            &projection,
        ),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_transcript_projection_validation_rejects_round_digest_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "transcript-digest");
    let mut transcript = Transcript::new("transcript-digest");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let mut projection = derive_ipa_verifier_transcript_projection::<PallasBackend>(
        params.n(),
        &mut transcript,
        &proof,
    )
    .expect("transcript projection derives");
    projection.rounds[0].round_bytes_digest[0] ^= 0x01;
    let mut validation = Transcript::new("transcript-digest");
    absorb_pallas_poly_statement(
        &mut validation,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    assert!(matches!(
        validate_ipa_verifier_transcript_projection::<PallasBackend>(
            params.n(),
            &mut validation,
            &proof,
            &projection,
        ),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_transcript_projection_validation_rejects_state_boundary_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "transcript-state");
    let mut transcript = Transcript::new("transcript-state");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let mut projection = derive_ipa_verifier_transcript_projection::<PallasBackend>(
        params.n(),
        &mut transcript,
        &proof,
    )
    .expect("transcript projection derives");
    projection.state_after_ipa_n[0] ^= 0x01;
    let mut validation = Transcript::new("transcript-state");
    absorb_pallas_poly_statement(
        &mut validation,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    assert!(matches!(
        validate_ipa_verifier_transcript_projection::<PallasBackend>(
            params.n(),
            &mut validation,
            &proof,
            &projection,
        ),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_transcript_projection_validation_rejects_round_order_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "transcript-order");
    let mut transcript = Transcript::new("transcript-order");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let mut projection = derive_ipa_verifier_transcript_projection::<PallasBackend>(
        params.n(),
        &mut transcript,
        &proof,
    )
    .expect("transcript projection derives");
    projection.rounds.swap(0, 1);
    let mut validation = Transcript::new("transcript-order");
    absorb_pallas_poly_statement(
        &mut validation,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    assert!(matches!(
        validate_ipa_verifier_transcript_projection::<PallasBackend>(
            params.n(),
            &mut validation,
            &proof,
            &projection,
        ),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_transcript_binding_rejects_zero_round_projection() {
    let projection = IpaVerifierTranscriptProjection::<pallas::Scalar> {
        n: 1,
        state_before_ipa_n: [0; 32],
        state_after_ipa_n: [0; 32],
        rounds: Vec::new(),
        final_state: [0; 32],
    };
    assert!(matches!(
        derive_ipa_verifier_transcript_binding(&projection),
        Err(Error::InvalidN(1))
    ));
}
#[test]
fn ipa_transcript_binding_projection_binds_rounds_and_challenges() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "transcript-binding");
    let mut transcript = Transcript::new("transcript-binding");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let projection = derive_ipa_verifier_transcript_projection::<PallasBackend>(
        params.n(),
        &mut transcript,
        &proof,
    )
    .expect("transcript projection derives");
    let binding =
        derive_ipa_verifier_transcript_binding(&projection).expect("transcript binding derives");
    assert_eq!(binding.n, params.n());
    assert_eq!(binding.round_projections.len(), 3);
    assert_eq!(binding.challenges.len(), 3);
    assert_eq!(binding.challenge_inverses.len(), 3);
    assert_ne!(binding.binding_digest.to_bytes(), [0u8; 32]);
    let mut recomputed = binding.header_projection;
    for round_index in 0..binding.round_projections.len() {
        recomputed = ipa_transcript_binding_round(
            recomputed,
            binding.round_projections[round_index],
            binding.challenges[round_index],
            binding.challenge_inverses[round_index],
        );
    }
    recomputed = ipa_transcript_binding_compress(recomputed, binding.final_projection);
    assert_eq!(recomputed.to_bytes(), binding.binding_digest.to_bytes());
    validate_ipa_verifier_transcript_binding(&projection, &binding)
        .expect("transcript binding validates");
}
#[test]
fn ipa_transcript_binding_validation_rejects_round_projection_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "binding-round");
    let mut transcript = Transcript::new("binding-round");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let projection = derive_ipa_verifier_transcript_projection::<PallasBackend>(
        params.n(),
        &mut transcript,
        &proof,
    )
    .expect("transcript projection derives");
    let mut binding =
        derive_ipa_verifier_transcript_binding(&projection).expect("transcript binding derives");
    binding.round_projections[0] = binding.round_projections[0].add(pallas::Scalar::one());
    assert!(matches!(
        validate_ipa_verifier_transcript_binding(&projection, &binding),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_transcript_binding_validation_rejects_challenge_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "binding-challenge");
    let mut transcript = Transcript::new("binding-challenge");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let projection = derive_ipa_verifier_transcript_projection::<PallasBackend>(
        params.n(),
        &mut transcript,
        &proof,
    )
    .expect("transcript projection derives");
    let mut binding =
        derive_ipa_verifier_transcript_binding(&projection).expect("transcript binding derives");
    binding.challenges[0] = binding.challenges[1];
    binding.challenge_inverses[0] = binding.challenge_inverses[1];
    assert!(matches!(
        validate_ipa_verifier_transcript_binding(&projection, &binding),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_transcript_binding_validation_rejects_digest_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "binding-digest");
    let mut transcript = Transcript::new("binding-digest");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let projection = derive_ipa_verifier_transcript_projection::<PallasBackend>(
        params.n(),
        &mut transcript,
        &proof,
    )
    .expect("transcript projection derives");
    let mut binding =
        derive_ipa_verifier_transcript_binding(&projection).expect("transcript binding derives");
    binding.binding_digest = binding.binding_digest.add(pallas::Scalar::one());
    assert!(matches!(
        validate_ipa_verifier_transcript_binding(&projection, &binding),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_b_vector_reduction_projection_matches_proof_final_b() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "b-reduction-projection");
    let mut transcript = Transcript::new("b-reduction-projection");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let rounds =
        derive_ipa_verifier_round_challenges::<PallasBackend>(params.n(), &mut transcript, &proof)
            .expect("round challenges derive");
    let b = pallas_evaluation_vector(params.n(), z);
    let reduction = derive_ipa_verifier_b_vector_reduction(&b, &rounds)
        .expect("b-vector reduction projection derives");
    assert_eq!(reduction.initial_b, b);
    assert_eq!(reduction.rounds.len(), 3);
    for (index, round) in reduction.rounds.iter().enumerate() {
        assert_eq!(round.round_index, index);
        assert_eq!(round.b_before.len(), 1 << (3 - index));
        assert_eq!(round.b_after.len(), 1 << (2 - index));
        assert_eq!(
            round.challenge.mul(round.challenge_inverse).to_bytes(),
            pallas::Scalar::one().to_bytes()
        );
        if index > 0 {
            assert_eq!(reduction.rounds[index - 1].b_after, round.b_before);
        }
    }
    assert_eq!(reduction.final_b.to_bytes(), proof.b_final.to_bytes());
}
#[test]
fn ipa_b_vector_reduction_projection_rejects_bad_shape() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "b-reduction-shape");
    let mut transcript = Transcript::new("b-reduction-shape");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let mut rounds =
        derive_ipa_verifier_round_challenges::<PallasBackend>(params.n(), &mut transcript, &proof)
            .expect("round challenges derive");
    let b = pallas_evaluation_vector(params.n(), z);
    assert!(matches!(
        derive_ipa_verifier_b_vector_reduction(&[pallas::Scalar::one()], &[]),
        Err(Error::InvalidN(1))
    ));
    assert!(matches!(
        derive_ipa_verifier_b_vector_reduction(&b[..7], &rounds),
        Err(Error::InvalidN(7))
    ));
    rounds.pop();
    assert!(matches!(
        derive_ipa_verifier_b_vector_reduction(&b, &rounds),
        Err(Error::InvalidProofShape {
            reason: "round challenge count",
            expected: 3,
            actual: 2,
        })
    ));
}
#[test]
fn ipa_b_vector_reduction_projection_rejects_round_index_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "b-reduction-index");
    let mut transcript = Transcript::new("b-reduction-index");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let mut rounds =
        derive_ipa_verifier_round_challenges::<PallasBackend>(params.n(), &mut transcript, &proof)
            .expect("round challenges derive");
    rounds[2].round_index = 9;
    let b = pallas_evaluation_vector(params.n(), z);
    assert!(matches!(
        derive_ipa_verifier_b_vector_reduction(&b, &rounds),
        Err(Error::InvalidProofShape {
            reason: "b-vector round challenge index",
            expected: 2,
            actual: 9,
        })
    ));
}
#[test]
fn ipa_b_vector_reduction_projection_rejects_inverse_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "b-reduction-inverse");
    let mut transcript = Transcript::new("b-reduction-inverse");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let mut rounds =
        derive_ipa_verifier_round_challenges::<PallasBackend>(params.n(), &mut transcript, &proof)
            .expect("round challenges derive");
    rounds[0].challenge = pallas::Scalar::from(2u64);
    rounds[0].challenge_inverse = pallas::Scalar::from(2u64);
    let b = pallas_evaluation_vector(params.n(), z);
    assert!(matches!(
        derive_ipa_verifier_b_vector_reduction(&b, &rounds),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_b_vector_reduction_projection_binds_challenge_values() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "b-reduction-binding");
    let mut transcript = Transcript::new("b-reduction-binding");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let mut rounds =
        derive_ipa_verifier_round_challenges::<PallasBackend>(params.n(), &mut transcript, &proof)
            .expect("round challenges derive");
    rounds[0].challenge = rounds[1].challenge;
    rounds[0].challenge_inverse = rounds[1].challenge_inverse;
    let b = pallas_evaluation_vector(params.n(), z);
    let reduction = derive_ipa_verifier_b_vector_reduction(&b, &rounds)
        .expect("tampered b reduction still has a well-shaped witness");
    assert_ne!(reduction.final_b.to_bytes(), proof.b_final.to_bytes());
}
#[test]
fn ipa_verifier_rejects_substituted_b_final() {
    let (params, z, commitment, t, mut proof) =
        sample_pallas_opening(8, "b-reduction-final-binding");
    proof.b_final = proof.b_final.add(pallas::Scalar::one());
    let mut transcript = Transcript::new("b-reduction-final-binding");
    let err = pallas::Polynomial::verify_open(&params, &mut transcript, z, commitment, t, &proof)
        .unwrap_err();
    assert!(matches!(err, Error::VerificationFailed));
}
#[test]
fn ipa_verifier_witness_projection_matches_verifier() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "witness-projection");
    let mut projected = Transcript::new("witness-projection");
    absorb_pallas_poly_statement(
        &mut projected,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let b = pallas_evaluation_vector(params.n(), z);
    let witness = derive_ipa_verifier_witness::<PallasBackend>(
        &params,
        &mut projected,
        &b,
        commitment,
        t,
        &proof,
    )
    .expect("recursive verifier witness derives");
    assert_eq!(witness.round_challenges.len(), 3);
    assert_eq!(
        witness.transcript_projection.rounds,
        witness.round_challenges
    );
    assert_eq!(witness.transcript_binding.n, params.n());
    assert_eq!(witness.transcript_binding.challenges.len(), 3);
    assert_eq!(
        witness.transcript_binding.challenges,
        witness
            .round_challenges
            .iter()
            .map(|round| round.challenge)
            .collect::<Vec<_>>()
    );
    assert_eq!(witness.b_reduction.final_b, proof.b_final);
    assert_eq!(witness.proof_a_final, proof.a_final);
    assert_eq!(witness.proof_b_final, proof.b_final);
    assert_eq!(
        witness.accumulation.final_q,
        witness.accumulation.expected_term
    );
    let mut validated = Transcript::new("witness-projection");
    absorb_pallas_poly_statement(
        &mut validated,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    validate_ipa_verifier_witness::<PallasBackend>(
        &params,
        &mut validated,
        &b,
        commitment,
        t,
        &proof,
        &witness,
    )
    .expect("native witness validates");
    let mut verified = Transcript::new("witness-projection");
    pallas::Polynomial::verify_open(&params, &mut verified, z, commitment, t, &proof)
        .expect("native verifier accepts proof");
    assert_eq!(projected.cur_digest(), verified.cur_digest());
}
#[test]
fn ipa_verifier_witness_validation_rejects_transcript_projection_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "witness-transcript");
    let mut transcript = Transcript::new("witness-transcript");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let b = pallas_evaluation_vector(params.n(), z);
    let mut witness = derive_ipa_verifier_witness::<PallasBackend>(
        &params,
        &mut transcript,
        &b,
        commitment,
        t,
        &proof,
    )
    .expect("recursive verifier witness derives");
    witness.transcript_projection.rounds[0].round_bytes_digest[0] ^= 0x01;
    let mut validation = Transcript::new("witness-transcript");
    absorb_pallas_poly_statement(
        &mut validation,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    assert!(matches!(
        validate_ipa_verifier_witness::<PallasBackend>(
            &params,
            &mut validation,
            &b,
            commitment,
            t,
            &proof,
            &witness,
        ),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_verifier_witness_validation_rejects_transcript_binding_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "witness-binding");
    let mut transcript = Transcript::new("witness-binding");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let b = pallas_evaluation_vector(params.n(), z);
    let mut witness = derive_ipa_verifier_witness::<PallasBackend>(
        &params,
        &mut transcript,
        &b,
        commitment,
        t,
        &proof,
    )
    .expect("recursive verifier witness derives");
    witness.transcript_binding.binding_digest = witness
        .transcript_binding
        .binding_digest
        .add(pallas::Scalar::one());
    let mut validation = Transcript::new("witness-binding");
    absorb_pallas_poly_statement(
        &mut validation,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    assert!(matches!(
        validate_ipa_verifier_witness::<PallasBackend>(
            &params,
            &mut validation,
            &b,
            commitment,
            t,
            &proof,
            &witness,
        ),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_verifier_witness_validation_rejects_transcript_challenge_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "witness-challenge");
    let mut transcript = Transcript::new("witness-challenge");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let b = pallas_evaluation_vector(params.n(), z);
    let mut witness = derive_ipa_verifier_witness::<PallasBackend>(
        &params,
        &mut transcript,
        &b,
        commitment,
        t,
        &proof,
    )
    .expect("recursive verifier witness derives");
    witness.round_challenges[0].challenge = witness.round_challenges[1].challenge;
    witness.round_challenges[0].challenge_inverse = witness.round_challenges[1].challenge_inverse;
    let mut validation = Transcript::new("witness-challenge");
    absorb_pallas_poly_statement(
        &mut validation,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    assert!(matches!(
        validate_ipa_verifier_witness::<PallasBackend>(
            &params,
            &mut validation,
            &b,
            commitment,
            t,
            &proof,
            &witness,
        ),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_verifier_witness_validation_rejects_b_reduction_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "witness-b-reduction");
    let mut transcript = Transcript::new("witness-b-reduction");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let b = pallas_evaluation_vector(params.n(), z);
    let mut witness = derive_ipa_verifier_witness::<PallasBackend>(
        &params,
        &mut transcript,
        &b,
        commitment,
        t,
        &proof,
    )
    .expect("recursive verifier witness derives");
    witness.b_reduction.rounds[0].b_after[0] =
        witness.b_reduction.rounds[0].b_after[0].add(pallas::Scalar::one());
    let mut validation = Transcript::new("witness-b-reduction");
    absorb_pallas_poly_statement(
        &mut validation,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    assert!(matches!(
        validate_ipa_verifier_witness::<PallasBackend>(
            &params,
            &mut validation,
            &b,
            commitment,
            t,
            &proof,
            &witness,
        ),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_verifier_witness_validation_rejects_accumulator_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "witness-accumulation");
    let mut transcript = Transcript::new("witness-accumulation");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let b = pallas_evaluation_vector(params.n(), z);
    let mut witness = derive_ipa_verifier_witness::<PallasBackend>(
        &params,
        &mut transcript,
        &b,
        commitment,
        t,
        &proof,
    )
    .expect("recursive verifier witness derives");
    witness.accumulation.rounds[0].q_after = witness.accumulation.rounds[0].q_before;
    let mut validation = Transcript::new("witness-accumulation");
    absorb_pallas_poly_statement(
        &mut validation,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    assert!(matches!(
        validate_ipa_verifier_witness::<PallasBackend>(
            &params,
            &mut validation,
            &b,
            commitment,
            t,
            &proof,
            &witness,
        ),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_verifier_witness_validation_rejects_final_scalar_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "witness-final-scalar");
    let mut transcript = Transcript::new("witness-final-scalar");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let b = pallas_evaluation_vector(params.n(), z);
    let mut witness = derive_ipa_verifier_witness::<PallasBackend>(
        &params,
        &mut transcript,
        &b,
        commitment,
        t,
        &proof,
    )
    .expect("recursive verifier witness derives");
    witness.proof_b_final = witness.proof_b_final.add(pallas::Scalar::one());
    let mut validation = Transcript::new("witness-final-scalar");
    absorb_pallas_poly_statement(
        &mut validation,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    assert!(matches!(
        validate_ipa_verifier_witness::<PallasBackend>(
            &params,
            &mut validation,
            &b,
            commitment,
            t,
            &proof,
            &witness,
        ),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_verifier_accumulation_projection_matches_verifier() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "accumulation-projection");
    let mut projected_transcript = Transcript::new("accumulation-projection");
    absorb_pallas_poly_statement(
        &mut projected_transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let rounds = derive_ipa_verifier_round_challenges::<PallasBackend>(
        params.n(),
        &mut projected_transcript,
        &proof,
    )
    .expect("round challenges derive");
    let b = pallas_evaluation_vector(params.n(), z);
    let accumulation = derive_ipa_verifier_accumulation::<PallasBackend>(
        &params, &b, commitment, t, &proof, &rounds,
    )
    .expect("accumulation projection derives");
    assert_eq!(accumulation.rounds.len(), 3);
    for (index, round) in accumulation.rounds.iter().enumerate() {
        assert_eq!(round.round_index, index);
        assert_eq!(round.g_after.len(), 1 << (2 - index));
        assert_eq!(round.h_after.len(), 1 << (2 - index));
        assert_eq!(
            round.challenge_square.to_bytes(),
            rounds[index]
                .challenge
                .mul(rounds[index].challenge)
                .to_bytes()
        );
        assert_eq!(
            round.challenge_inverse_square.to_bytes(),
            rounds[index]
                .challenge_inverse
                .mul(rounds[index].challenge_inverse)
                .to_bytes()
        );
        if index > 0 {
            assert_eq!(accumulation.rounds[index - 1].q_after, round.q_before);
        }
    }
    assert_eq!(accumulation.final_q, accumulation.expected_term);
    assert_eq!(accumulation.final_g, accumulation.rounds[2].g_after[0]);
    assert_eq!(accumulation.final_h, accumulation.rounds[2].h_after[0]);
    let mut verified = Transcript::new("accumulation-projection");
    pallas::Polynomial::verify_open(&params, &mut verified, z, commitment, t, &proof)
        .expect("native verifier accepts proof");
}
#[test]
fn ipa_verifier_accumulation_projection_rejects_bad_challenge_shape() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "accumulation-shape");
    let mut transcript = Transcript::new("accumulation-shape");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let mut rounds =
        derive_ipa_verifier_round_challenges::<PallasBackend>(params.n(), &mut transcript, &proof)
            .expect("round challenges derive");
    let b = pallas_evaluation_vector(params.n(), z);
    rounds.pop();
    assert!(matches!(
        derive_ipa_verifier_accumulation::<PallasBackend>(
            &params, &b, commitment, t, &proof, &rounds,
        ),
        Err(Error::InvalidProofShape {
            reason: "round challenge count",
            expected: 3,
            actual: 2,
        })
    ));
}
#[test]
fn ipa_verifier_accumulation_projection_rejects_round_index_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "accumulation-index");
    let mut transcript = Transcript::new("accumulation-index");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let mut rounds =
        derive_ipa_verifier_round_challenges::<PallasBackend>(params.n(), &mut transcript, &proof)
            .expect("round challenges derive");
    rounds[1].round_index = 7;
    let b = pallas_evaluation_vector(params.n(), z);
    assert!(matches!(
        derive_ipa_verifier_accumulation::<PallasBackend>(
            &params, &b, commitment, t, &proof, &rounds,
        ),
        Err(Error::InvalidProofShape {
            reason: "round challenge index",
            expected: 1,
            actual: 7,
        })
    ));
}
#[test]
fn ipa_verifier_accumulation_projection_rejects_inverse_substitution() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "accumulation-inverse");
    let mut transcript = Transcript::new("accumulation-inverse");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let mut rounds =
        derive_ipa_verifier_round_challenges::<PallasBackend>(params.n(), &mut transcript, &proof)
            .expect("round challenges derive");
    rounds[0].challenge = pallas::Scalar::from(2u64);
    rounds[0].challenge_inverse = pallas::Scalar::from(2u64);
    let b = pallas_evaluation_vector(params.n(), z);
    assert!(matches!(
        derive_ipa_verifier_accumulation::<PallasBackend>(
            &params, &b, commitment, t, &proof, &rounds,
        ),
        Err(Error::VerificationFailed)
    ));
}
#[test]
fn ipa_verifier_accumulation_projection_binds_challenge_values() {
    let (params, z, commitment, t, proof) = sample_pallas_opening(8, "accumulation-binding");
    let mut transcript = Transcript::new("accumulation-binding");
    absorb_pallas_poly_statement(
        &mut transcript,
        &params,
        z,
        commitment,
        t,
        PolyOpenTranscriptMetadata::default(),
    );
    let mut rounds =
        derive_ipa_verifier_round_challenges::<PallasBackend>(params.n(), &mut transcript, &proof)
            .expect("round challenges derive");
    rounds[0].challenge = rounds[1].challenge;
    rounds[0].challenge_inverse = rounds[1].challenge_inverse;
    let b = pallas_evaluation_vector(params.n(), z);
    let accumulation = derive_ipa_verifier_accumulation::<PallasBackend>(
        &params, &b, commitment, t, &proof, &rounds,
    )
    .expect("tampered accumulation still has a well-shaped witness");
    assert_ne!(accumulation.final_q, accumulation.expected_term);
}
#[test]
fn batch_verify_rejects_tampered_bound_public_claims() {
    let params = pallas::Params::new(8).unwrap();
    let coeffs = sample_pallas_coeffs(8);
    let poly = pallas::Polynomial::from_coeffs(coeffs);
    let commitment = poly.commit(&params).unwrap();
    let z = pallas::Scalar::from(11u64);
    let metadata = PolyOpenTranscriptMetadata {
        vk_commitment: Some([0x11; 32]),
        public_inputs_schema_hash: Some([0x22; 32]),
        domain_tag: Some([0x33; 32]),
    };
    let mut transcript = Transcript::new("bound-claims");
    let (proof, t) = poly
        .open_with_metadata(&params, &mut transcript, z, commitment, metadata)
        .unwrap();
    let envelope = OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, commitment),
        proof: nh::proof_to_wire(&proof),
        transcript_label: "bound-claims".into(),
        vk_commitment: metadata.vk_commitment,
        public_inputs_schema_hash: metadata.public_inputs_schema_hash,
        domain_tag: metadata.domain_tag,
    };
    let ok = crate::batch::verify_open_batch(std::slice::from_ref(&envelope));
    assert!(matches!(ok[0], Ok(true)));
    let mut bad_z = envelope.clone();
    bad_z.public.z[0] ^= 0x01;
    let mut bad_t = envelope.clone();
    bad_t.public.t[0] ^= 0x01;
    let mut bad_commitment = envelope.clone();
    bad_commitment.public.p_g[0] ^= 0x01;
    let mut bad_label = envelope.clone();
    bad_label.transcript_label.push_str("-tampered");
    let mut bad_vk = envelope.clone();
    bad_vk.vk_commitment = Some([0x44; 32]);
    let mut bad_schema = envelope.clone();
    bad_schema.public_inputs_schema_hash = Some([0x55; 32]);
    let mut bad_domain = envelope.clone();
    bad_domain.domain_tag = Some([0x66; 32]);
    for tampered in [
        bad_z,
        bad_t,
        bad_commitment,
        bad_label,
        bad_vk,
        bad_schema,
        bad_domain,
    ] {
        let result = crate::batch::verify_open_batch(std::slice::from_ref(&tampered));
        assert!(
            !matches!(result[0], Ok(true)),
            "tampered envelope unexpectedly verified: {tampered:?}"
        );
    }
}
#[test]
fn pallas_open_envelope_derives_verifier_witness() {
    let envelope = sample_pallas_envelope(8, "derive-envelope-witness");
    let (params, witness) =
        nh::derive_pallas_ipa_verifier_witness_from_envelope(&envelope).unwrap();
    let nh::DecodedEnvelope::Pallas {
        params: decoded_params,
        proof,
        z,
        t,
        p_g,
    } = nh::decode_envelope(&envelope).unwrap()
    else {
        panic!("sample envelope must decode as Pallas");
    };
    assert_eq!(params.fingerprint(), decoded_params.fingerprint());
    assert_eq!(witness.transcript_projection.n, params.n());
    assert_eq!(
        witness.accumulation.final_q,
        witness.accumulation.expected_term
    );
    let b = pallas_evaluation_vector(params.n(), z);
    let mut transcript = Transcript::new(&envelope.transcript_label);
    absorb_pallas_poly_statement(
        &mut transcript,
        params.as_ref(),
        z,
        p_g,
        t,
        envelope.transcript_metadata(),
    );
    validate_ipa_verifier_witness::<PallasBackend>(
        params.as_ref(),
        &mut transcript,
        &b,
        p_g,
        t,
        proof.as_ref(),
        &witness,
    )
    .unwrap();
}
#[test]
fn pallas_open_envelope_witness_derivation_honors_limits() {
    let envelope = sample_pallas_envelope(8, "derive-envelope-witness-limits");
    nh::derive_pallas_ipa_verifier_witness_from_envelope_with_limits(
        &envelope,
        OpenVerifyLimits::new(3, usize::MAX),
    )
    .expect("n=8 envelope is inside max_k=3");
    let err = nh::derive_pallas_ipa_verifier_witness_from_envelope_with_limits(
        &envelope,
        OpenVerifyLimits::new(2, usize::MAX),
    )
    .expect_err("n=8 envelope exceeds max_k=2");
    assert!(matches!(
        err,
        Error::EnvelopeLimitExceeded {
            limit: "max_k",
            max: 4,
            actual: 8
        }
    ));
}
#[test]
fn pallas_open_envelope_witness_derivation_rejects_tampering() {
    let envelope = sample_pallas_envelope(8, "derive-envelope-witness-tamper");
    let mut bad_label = envelope.clone();
    bad_label.transcript_label.push_str("-forged");
    assert!(nh::derive_pallas_ipa_verifier_witness_from_envelope(&bad_label).is_err());
    let mut bad_claim = envelope.clone();
    bad_claim.public.t[0] ^= 0x01;
    assert!(nh::derive_pallas_ipa_verifier_witness_from_envelope(&bad_claim).is_err());
    let mut bad_proof = envelope;
    bad_proof.proof.a_final[0] ^= 0x01;
    assert!(nh::derive_pallas_ipa_verifier_witness_from_envelope(&bad_proof).is_err());
}
