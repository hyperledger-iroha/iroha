#![doc = "Native IPA verifier (no external halo2) end-to-end tests."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]
//! Native IPA verifier (no external halo2) end-to-end tests.

#![cfg(feature = "zk-ipa-native")]

use iroha_core::zk::verify_backend;
use iroha_data_model::proof::ProofBox;

#[test]
fn ipa_poly_open_roundtrip_ok_and_fail() {
    use iroha_zkp_halo2::{
        OpenVerifyEnvelope, Params, Polynomial, PrimeField64, Transcript,
        backend::pallas::PallasBackend, norito_helpers as nh,
    };

    // Deterministic parameters and polynomial of length 8
    let n = 8usize;
    let params = Params::new(n).expect("params");
    let coeffs = (0..n)
        .map(|i| PrimeField64::from((i as u64) + 1))
        .collect::<Vec<_>>();
    let poly = Polynomial::from_coeffs(coeffs);
    let p_g = poly.commit(&params).expect("commit");

    // Open at z=3 and build envelope
    let z = PrimeField64::from(3u64);
    let mut tr = Transcript::new("IROHA-TEST-IPA");
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).expect("open");
    let mut tr_v = Transcript::new("IROHA-TEST-IPA");
    // Verify via native API to sanity-check test vector
    iroha_zkp_halo2::Polynomial::verify_open(&params, &mut tr_v, z, p_g, t, &proof)
        .expect("native verify");

    let env = OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(n, z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: "IROHA-TEST-IPA".to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let bytes = norito::to_bytes(&env).expect("encode");
    let pb_ok = ProofBox::new("halo2/ipa-v1/poly-open".into(), bytes.clone());
    assert!(verify_backend("halo2/ipa-v1/poly-open", &pb_ok, None));

    // Corrupt t in the envelope and expect verification failure
    let mut env_bad = env.clone();
    env_bad.public.t[0] ^= 1;
    let bad_bytes = norito::to_bytes(&env_bad).expect("encode");
    let pb_bad = ProofBox::new("halo2/ipa-v1/poly-open".into(), bad_bytes);
    assert!(!verify_backend("halo2/ipa-v1/poly-open", &pb_bad, None));
}
