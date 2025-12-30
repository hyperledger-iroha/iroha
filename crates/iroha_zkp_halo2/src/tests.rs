//! Unit tests validating the deterministic IPA and polynomial opening flow.

use core::num::NonZeroUsize;

use super::*;
#[cfg(feature = "goldilocks_backend")]
use crate::backend::IpaGroup;
#[cfg(feature = "goldilocks_backend")]
use crate::backend::goldilocks::{self as gold, GoldilocksBackend};
use crate::{
    backend::{
        bn254::{self as bn254, Bn254Backend},
        pallas::{self as pallas, PallasBackend},
    },
    errors::Error,
    norito_helpers as nh,
};

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
