//! Unit tests validating the deterministic IPA and polynomial opening flow.

use core::num::NonZeroUsize;

use super::*;
#[cfg(feature = "goldilocks_backend")]
use crate::backend::IpaGroup;
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
fn params_invalid_n() {
    assert!(pallas::Params::new(0).is_err());
    assert!(pallas::Params::new(3).is_err());
    #[cfg(feature = "goldilocks_backend")]
    {
        assert!(gold::Params::new(0).is_err());
        assert!(gold::Params::new(3).is_err());
    }
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
}

#[test]
fn params_from_wire_rejects_tampered_pallas_generators() {
    let params = pallas::Params::new(8).unwrap();
    let mut wire = nh::params_to_wire(&params);
    wire.g[0] = pallas::Group::identity().to_bytes();
    let err = nh::params_from_wire::<PallasBackend>(&wire).unwrap_err();
    assert!(matches!(err, Error::InvalidGenerator { kind: "G", .. }));
}

#[test]
fn params_from_wire_rejects_bad_generator_lengths_before_curve_decoding() {
    let params = pallas::Params::new(8).unwrap();
    let mut wire = nh::params_to_wire(&params);
    wire.g.pop();
    let err = nh::params_from_wire::<PallasBackend>(&wire).unwrap_err();
    assert!(matches!(
        err,
        Error::DimensionMismatch {
            expected: 8,
            actual: 7
        }
    ));
}

#[test]
fn params_registry_is_bounded_and_refreshes_recent_entries() {
    let _guard = PARAMS_REGISTRY_TEST_LOCK.lock().unwrap();
    crate::params::clear_params_registry_for_tests();

    let n = (crate::params::PARAMS_REGISTRY_MAX_ENTRIES + 2).next_power_of_two();
    let base = pallas::Params::new(n).unwrap();
    let mut wires = Vec::new();
    for rotation in 0..=crate::params::PARAMS_REGISTRY_MAX_ENTRIES {
        let mut wire = nh::params_to_wire(&base);
        wire.g.rotate_left(rotation % n);
        wire.h.rotate_right(rotation % n);
        wires.push(wire);
    }

    for wire in wires
        .iter()
        .take(crate::params::PARAMS_REGISTRY_MAX_ENTRIES)
    {
        nh::params_from_wire::<PallasBackend>(wire).unwrap();
    }
    assert_eq!(
        crate::params::params_registry_len_for_tests(),
        crate::params::PARAMS_REGISTRY_MAX_ENTRIES
    );

    let first_fp = wires[0].fingerprint();
    let second_fp = wires[1].fingerprint();
    nh::params_from_wire::<PallasBackend>(&wires[0]).unwrap();
    nh::params_from_wire::<PallasBackend>(&wires[crate::params::PARAMS_REGISTRY_MAX_ENTRIES])
        .unwrap();

    assert_eq!(
        crate::params::params_registry_len_for_tests(),
        crate::params::PARAMS_REGISTRY_MAX_ENTRIES
    );
    assert!(crate::params::params_registry_contains_for_tests::<
        PallasBackend,
    >(&first_fp));
    assert!(!crate::params::params_registry_contains_for_tests::<
        PallasBackend,
    >(&second_fp));

    crate::params::clear_params_registry_for_tests();
}

#[test]
fn params_registry_keys_include_backend_curve() {
    let _guard = PARAMS_REGISTRY_TEST_LOCK.lock().unwrap();
    crate::params::clear_params_registry_for_tests();

    let pallas_wire = nh::params_to_wire(&pallas::Params::new(8).unwrap());
    let bn254_wire = nh::params_to_wire(&bn254::Params::new(8).unwrap());
    let pallas_fp = pallas_wire.fingerprint();
    let bn254_fp = bn254_wire.fingerprint();

    nh::params_from_wire::<PallasBackend>(&pallas_wire).unwrap();
    nh::params_from_wire::<Bn254Backend>(&bn254_wire).unwrap();

    assert!(crate::params::params_registry_contains_for_tests::<
        PallasBackend,
    >(&pallas_fp));
    assert!(crate::params::params_registry_contains_for_tests::<
        Bn254Backend,
    >(&bn254_fp));

    crate::params::clear_params_registry_for_tests();
}

#[test]
fn params_registry_reuses_registered_wire_params() {
    let _guard = PARAMS_REGISTRY_TEST_LOCK.lock().unwrap();
    crate::params::clear_params_registry_for_tests();

    let wire = nh::params_to_wire(&pallas::Params::new(8).unwrap());
    let registered = nh::register_params_from_wire::<PallasBackend>(&wire).unwrap();
    let decoded = nh::params_from_wire::<PallasBackend>(&wire).unwrap();

    assert!(std::sync::Arc::ptr_eq(&registered, &decoded));

    crate::params::clear_params_registry_for_tests();
}

#[test]
fn params_from_wire_rejects_duplicate_generators() {
    let params = pallas::Params::new(8).unwrap();

    let mut wire = nh::params_to_wire(&params);
    wire.g[1] = wire.g[0];
    let err = nh::params_from_wire::<PallasBackend>(&wire).unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidGenerator {
            kind: "G",
            index: 1,
            ..
        }
    ));

    let mut wire = nh::params_to_wire(&params);
    wire.h[1] = wire.h[0];
    let err = nh::params_from_wire::<PallasBackend>(&wire).unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidGenerator {
            kind: "H",
            index: 1,
            ..
        }
    ));
}

#[test]
fn params_from_wire_rejects_tampered_bn254_generators() {
    let params = bn254::Params::new(8).unwrap();
    let mut wire = nh::params_to_wire(&params);
    wire.h[0] = bn254::GroupElem::identity().to_bytes();
    let err = nh::params_from_wire::<Bn254Backend>(&wire).unwrap_err();
    assert!(matches!(err, Error::InvalidGenerator { kind: "H", .. }));
}

#[cfg(feature = "goldilocks_backend")]
#[test]
fn params_from_wire_rejects_tampered_goldilocks_generators() {
    let params = gold::Params::new(8).unwrap();
    let mut wire = nh::params_to_wire(&params);
    let identity = <gold::Group as IpaGroup>::identity();
    wire.h[0] = identity.to_bytes();
    let err = nh::params_from_wire::<GoldilocksBackend>(&wire).unwrap_err();
    assert!(matches!(err, Error::InvalidGenerator { kind: "H", .. }));
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
    assert!(matches!(results[1], Ok(false)));

    let seq_results = crate::batch::verify_open_batch_with_options(
        &[env_ok.clone(), env_bad.clone()],
        &crate::batch::BatchOptions::sequential(),
    );
    for (lhs, rhs) in results.iter().zip(seq_results.iter()) {
        assert_eq!(lhs.as_ref().unwrap(), rhs.as_ref().unwrap());
    }

    let limited_results = crate::batch::verify_open_batch_with_options(
        &[env_ok, env_bad],
        &crate::batch::BatchOptions::limited(NonZeroUsize::new(1).unwrap()),
    );
    for (lhs, rhs) in seq_results.iter().zip(limited_results.iter()) {
        assert_eq!(lhs.as_ref().unwrap(), rhs.as_ref().unwrap());
    }
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
    let decoded = nh::decode_envelope_with_limits(
        &envelope,
        OpenVerifyLimits {
            max_k: Some(3),
            max_transcript_label_len: Some(4),
        },
    )
    .unwrap();
    assert!(matches!(decoded, nh::DecodedEnvelope::Pallas { .. }));
}

#[test]
fn decode_envelope_accepts_large_max_k_limit() {
    let envelope = sample_pallas_envelope(8, "large-max-k");
    let decoded = nh::decode_envelope_with_limits(
        &envelope,
        OpenVerifyLimits {
            max_k: Some(usize::BITS),
            max_transcript_label_len: Some("large-max-k".len()),
        },
    )
    .unwrap();
    assert!(matches!(decoded, nh::DecodedEnvelope::Pallas { .. }));
}

#[test]
fn params_from_wire_reports_version_and_curve_mismatch() {
    let params = pallas::Params::new(8).unwrap();
    let wire = nh::params_to_wire(&params);

    let mut bad = wire.clone();
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
    let limits = OpenVerifyLimits {
        max_k: Some(0),
        max_transcript_label_len: Some(0),
    };

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
        OpenVerifyLimits {
            max_k: Some(2),
            max_transcript_label_len: Some(64),
        },
    );
    assert!(matches!(
        results[0],
        Err(Error::EnvelopeLimitExceeded { limit: "max_k", .. })
    ));

    let results = crate::batch::verify_open_batch_with_limits(
        std::slice::from_ref(&envelope),
        &crate::batch::BatchOptions::sequential(),
        OpenVerifyLimits {
            max_k: Some(3),
            max_transcript_label_len: Some(4),
        },
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
