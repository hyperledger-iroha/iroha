//! ZK IPA Norito envelope end-to-end tests against the `verify_open_envelope` helper.

use ivm::zk_verify::verify_open_envelope;

#[test]
fn ipa_open_envelope_verifies() {
    use h2::{PrimeField64 as F, norito_helpers as nh};
    use iroha_zkp_halo2::{self as h2, backend::pallas::PallasBackend};

    // Build params and a simple polynomial f(x) with deterministic coefficients
    let params = h2::Params::new(8).expect("params new");
    let coeffs: Vec<F> = (0u64..8).map(|i| F::from(i + 1)).collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);

    // Commit and open at z
    let mut tr = h2::Transcript::new("zk-open-test");
    let p_g = poly.commit(&params).expect("commit");
    let z = F::from(5u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).expect("open");

    // Compose Norito envelope
    let params_wire = nh::params_to_wire(&params);
    let public_wire = nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g);
    let proof_wire = nh::proof_to_wire(&proof);
    let env = h2::OpenVerifyEnvelope {
        params: params_wire,
        public: public_wire,
        proof: proof_wire,
        transcript_label: "zk-open-test".to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let bytes = norito::to_bytes(&env).expect("encode");

    // Verify via IVM helper
    let ok = verify_open_envelope(&bytes).expect("verifier ran");
    assert!(ok, "expected verification success");
}

#[test]
fn ipa_open_envelope_rejects_wrong_t() {
    use h2::{PrimeField64 as F, norito_helpers as nh};
    use iroha_zkp_halo2::{self as h2, backend::pallas::PallasBackend};

    let params = h2::Params::new(8).expect("params new");
    let coeffs: Vec<F> = (0u64..8).map(|i| F::from(i + 1)).collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new("zk-open-test");
    let p_g = poly.commit(&params).expect("commit");
    let z = F::from(7u64);
    let (proof, _t) = poly.open(&params, &mut tr, z, p_g).expect("open");

    // Intentionally use a wrong t value
    let wrong_t = F::from(999u64);

    let params_wire = nh::params_to_wire(&params);
    let public_wire = nh::poly_open_public::<PallasBackend>(params.n(), z, wrong_t, p_g);
    let proof_wire = nh::proof_to_wire(&proof);
    let env = h2::OpenVerifyEnvelope {
        params: params_wire,
        public: public_wire,
        proof: proof_wire,
        transcript_label: "zk-open-test".to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let bytes = norito::to_bytes(&env).expect("encode");

    // The helper maps a failed verification to Ok(false)
    let ok = verify_open_envelope(&bytes).expect("verifier ran");
    assert!(!ok, "expected verification failure on wrong evaluation");
}

#[test]
fn ipa_open_envelope_rejects_transcript_label_mismatch() {
    use h2::{PrimeField64 as F, norito_helpers as nh};
    use iroha_zkp_halo2::{self as h2, backend::pallas::PallasBackend};

    // Build proof with transcript "label-a" but embed envelope with "label-b"
    let params = h2::Params::new(8).expect("params new");
    let coeffs: Vec<F> = (0u64..8).map(|i| F::from(i + 1)).collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new("label-a");
    let p_g = poly.commit(&params).expect("commit");
    let z = F::from(3u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).expect("open");

    let params_wire = nh::params_to_wire(&params);
    let public_wire = nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g);
    let proof_wire = nh::proof_to_wire(&proof);
    // Embed a different transcript label to force a verification failure
    let env = h2::OpenVerifyEnvelope {
        params: params_wire,
        public: public_wire,
        proof: proof_wire,
        transcript_label: "label-b".to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let bytes = norito::to_bytes(&env).expect("encode");
    let ok = ivm::zk_verify::verify_open_envelope(&bytes).expect("verifier ran");
    assert!(!ok, "expected verification failure on transcript mismatch");
}
