//! Determinism and corruption checks for Halo2 OpenVerifyEnvelope verification.
//!
//! Exercises the production verifier (not the `MockProver`) by generating a real
//! IPA proof, verifying it twice for determinism, and then corrupting the proof
//! to ensure verification fails cleanly instead of panicking.

use iroha_zkp_halo2 as h2;
use iroha_zkp_halo2::{
    OpenVerifyEnvelope,
    backend::{bn254::Bn254Backend, pallas::PallasBackend},
    norito_helpers as nh,
};

fn build_envelope(label: &str) -> OpenVerifyEnvelope {
    // Small polynomial over ToyP61 with a fixed transcript label to keep proof
    // generation deterministic.
    let params = h2::Params::new(8).expect("params");
    let coeffs: Vec<h2::PrimeField64> = (0u64..params.n() as u64)
        .map(|i| h2::PrimeField64::from(i + 1))
        .collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new(label);
    let p_g = poly.commit(&params).expect("commit");
    let z = h2::PrimeField64::from(5u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).expect("open");
    OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: label.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    }
}

#[test]
fn verify_open_envelope_is_deterministic_and_rejects_corruption() {
    let env = build_envelope("deterministic-proof");
    let bytes = norito::to_bytes(&env).expect("encode env");

    // Deterministic: repeated verification of the same envelope must succeed.
    assert!(ivm::zk_verify::verify_open_envelope(&bytes).expect("verify 1"));
    assert!(ivm::zk_verify::verify_open_envelope(&bytes).expect("verify 2"));

    // Corrupt a limb in the proof while keeping the envelope well-formed.
    let mut corrupted = env.clone();
    if let Some(first_l) = corrupted.proof.l.first_mut() {
        first_l[0] ^= 0xAA;
    } else {
        panic!("expected non-empty proof.l");
    }
    let bad_bytes = norito::to_bytes(&corrupted).expect("encode corrupted env");
    match ivm::zk_verify::verify_open_envelope(&bad_bytes) {
        Ok(result) => assert!(
            !result,
            "corrupted proof must fail verification without panicking"
        ),
        Err(_) => {
            // Accept well-formed decode errors as a deterministic rejection path.
        }
    }
}

#[test]
fn verify_bn254_envelope_roundtrip_and_corruption() {
    let params = h2::Bn254Params::new(8).expect("params");
    let coeffs: Vec<h2::Bn254Scalar> = (0..params.n())
        .map(|i| h2::Bn254Scalar::from((i as u64) + 1))
        .collect();
    let poly = h2::Bn254Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new("bn254-proof");
    let p_g = poly.commit(&params).expect("commit");
    let z = h2::Bn254Scalar::from(3u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).expect("open");
    let env = OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<Bn254Backend>(params.n(), z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: "bn254-proof".to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };

    let bytes = norito::to_bytes(&env).expect("encode env");
    assert!(ivm::zk_verify::verify_open_envelope(&bytes).expect("verify bn254 ok"));

    let mut corrupted = env.clone();
    if let Some(first_r) = corrupted.proof.r.first_mut() {
        first_r[0] ^= 0x55;
    } else {
        panic!("expected non-empty proof.r");
    }
    let bad_bytes = norito::to_bytes(&corrupted).expect("encode corrupted env");
    match ivm::zk_verify::verify_open_envelope(&bad_bytes) {
        Ok(result) => assert!(!result, "corrupted proof must fail verification"),
        Err(_) => {
            // Decode errors are an acceptable deterministic rejection.
        }
    }
}
