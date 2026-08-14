//! Exact-lock release guards for the first-party Microsoft Vega-MC boundary.
use iroha_zkp_halo2::vega::{
    VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1, VegaMdlProofErrorV1, vega_mdl_proof_dimensions_v1,
    vega_mdl_verifier_digest_v1, vega_microsoft_fixture_conformance_v1,
};
const CRATE_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
const VEGA_FACADE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega.rs"));
const EXACT_BOUNDARY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/vega/canonical_mc_exact.rs"
));
const PYTHON_VERIFIER_KEY: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../vendor/vega-prover/reference/fixtures/cubic/python_vk.bin"
));
const PYTHON_PROOF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../vendor/vega-prover/reference/fixtures/cubic/python_standalone_proof.bin"
));
#[test]
fn public_profile_identity_and_dimensions_remain_frozen() {
    assert_eq!(
        vega_mdl_verifier_digest_v1().expect("governed verifier identity"),
        VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1
    );
    let dimensions = vega_mdl_proof_dimensions_v1().expect("governed dimensions");
    assert_eq!(dimensions.num_steps, 8);
    assert_eq!(dimensions.shared_variables, 524_288);
    assert_eq!(dimensions.step_variables, 1_048_576);
    assert_eq!(dimensions.core_variables, 1_048_576);
    assert_eq!(dimensions.verifier_round_commitment_points, [1; 47]);
    assert_eq!(dimensions.verifier_challenges_per_round.len(), 47);
    assert_eq!(dimensions.relaxed_outer_rounds, 9);
    assert_eq!(dimensions.relaxed_inner_rounds, 12);
}
#[test]
fn production_boundary_cannot_compile_or_route_the_oracle_crates() {
    for forbidden_edge in [
        "bellpepper-core =",
        "bellpepper =",
        "ff =",
        "vega-prover =",
        "bincode =",
        "sha2 =",
    ] {
        assert!(
            !CRATE_MANIFEST.contains(forbidden_edge),
            "exact-lock-breaking edge remains: {forbidden_edge}"
        );
    }
    assert!(CRATE_MANIFEST.contains("rayon = { workspace = true, optional = true }"));
    assert!(CRATE_MANIFEST.contains("parallel = [\"dep:rayon\"]"));
    assert!(VEGA_FACADE.contains("#[path = \"vega/canonical_mc_exact.rs\"]"));
    for forbidden_path in [
        "vega_prover",
        "bellpepper",
        "MaskedRelaxedProofWire",
        "prove_masked_relaxed",
        "verify_masked_relaxed",
    ] {
        assert!(
            !EXACT_BOUNDARY.contains(forbidden_path),
            "production boundary contains a substituted path: {forbidden_path}"
        );
    }
}
#[test]
fn first_party_verifier_accepts_the_independent_python_fixture() {
    assert_eq!(PYTHON_VERIFIER_KEY.len(), 200_292);
    assert_eq!(PYTHON_PROOF.len(), 73_484);
    let (digest, dimensions, step_outputs, core_outputs) =
        vega_microsoft_fixture_conformance_v1(PYTHON_VERIFIER_KEY, PYTHON_PROOF)
            .expect("first-party verifier accepts the independent Microsoft fixture");
    assert_eq!(
        digest,
        hex_literal::hex!("b752511606285b40d5a1ea19ba3f6b4e7d6f90cc29036cf4b59cfd5121dc2729")
    );
    assert_eq!(dimensions.num_steps, 2);
    assert_eq!(step_outputs, 2);
    assert_eq!(core_outputs, 1);
}
#[test]
fn first_party_codec_rejects_truncated_and_trailing_bytes() {
    assert!(
        vega_microsoft_fixture_conformance_v1(
            &PYTHON_VERIFIER_KEY[..PYTHON_VERIFIER_KEY.len() - 1],
            PYTHON_PROOF,
        )
        .is_err()
    );
    assert!(
        vega_microsoft_fixture_conformance_v1(
            PYTHON_VERIFIER_KEY,
            &PYTHON_PROOF[..PYTHON_PROOF.len() - 1],
        )
        .is_err()
    );
    let mut trailing_key = PYTHON_VERIFIER_KEY.to_vec();
    trailing_key.push(0);
    assert!(vega_microsoft_fixture_conformance_v1(&trailing_key, PYTHON_PROOF).is_err());
    let mut trailing_proof = PYTHON_PROOF.to_vec();
    trailing_proof.push(0);
    assert!(vega_microsoft_fixture_conformance_v1(PYTHON_VERIFIER_KEY, &trailing_proof).is_err());
}
#[test]
fn first_party_verifier_rejects_equation_corruption() {
    let mut corrupted = PYTHON_PROOF.to_vec();
    let final_scalar_low_byte = corrupted
        .len()
        .checked_sub(32)
        .expect("fixture contains its final scalar");
    corrupted[final_scalar_low_byte] ^= 1;
    assert_eq!(
        vega_microsoft_fixture_conformance_v1(PYTHON_VERIFIER_KEY, &corrupted),
        Err(VegaMdlProofErrorV1::VerificationFailed)
    );
}
