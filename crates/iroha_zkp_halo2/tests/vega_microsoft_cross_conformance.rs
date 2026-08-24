//! Exact-lock release guards for the first-party Microsoft Vega-MC boundary.
use iroha_zkp_halo2::vega::{
    VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1, VegaMdlProofErrorV1,
    install_vega_mdl_figure9_prover_artifacts_v1, install_vega_mdl_figure9_verifier_key_v1,
    vega_mdl_proof_dimensions_v1, vega_mdl_verifier_digest_v1,
};
const CRATE_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
const VEGA_FACADE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega.rs"));
const EXACT_BOUNDARY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/vega/canonical_mc_exact.rs"
));
const MICROSOFT_MC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/vega/microsoft_mc.rs"
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
fn governed_figure9_key_install_is_explicit_strict_and_has_no_ambient_provider() {
    assert_eq!(
        install_vega_mdl_figure9_verifier_key_v1(PYTHON_VERIFIER_KEY),
        Err(VegaMdlProofErrorV1::InvalidCompiledProfile)
    );
    assert_eq!(
        install_vega_mdl_figure9_prover_artifacts_v1(&[], PYTHON_VERIFIER_KEY),
        Err(VegaMdlProofErrorV1::InvalidCompiledProfile)
    );
    let production_boundary = EXACT_BOUNDARY
        .split_once("#[cfg(test)]")
        .expect("production/test boundary")
        .0;
    for forbidden_lookup in [
        "std::env",
        "std::fs",
        "option_env!",
        "reqwest",
        "ureq",
        "http://",
        "https://",
    ] {
        assert!(
            !production_boundary.contains(forbidden_lookup)
                && !MICROSOFT_MC.contains(forbidden_lookup),
            "governed key path gained an ambient lookup: {forbidden_lookup}"
        );
    }
    assert!(MICROSOFT_MC.contains("static GOVERNED_FIGURE9_ARTIFACTS"));
    assert!(MICROSOFT_MC.contains("OnceCell<verifier_key::McVerifierKeyWire>"));
    assert!(MICROSOFT_MC.contains("OnceCell<prover_key::McProverKeyWire>"));
    assert!(MICROSOFT_MC.contains("install_lock: Mutex<()>"));
    assert!(!MICROSOFT_MC.contains("take(&mut self)"));
}
#[test]
fn first_party_verifier_accepts_the_independent_python_fixture() {
    assert_eq!(PYTHON_VERIFIER_KEY.len(), 200_292);
    assert_eq!(PYTHON_PROOF.len(), 73_484);
    assert!(!VEGA_FACADE.contains("vega_microsoft_fixture_conformance_v1"));
    assert!(!EXACT_BOUNDARY.contains("validate_microsoft_fixture"));
    assert!(!MICROSOFT_MC.contains("pub(super) fn validate_fixture"));
    assert!(MICROSOFT_MC.contains("#[cfg(test)]\ntype ValidatedFixture ="));
    assert!(MICROSOFT_MC.contains("#[cfg(test)]\nfn validate_fixture("));
    assert!(MICROSOFT_MC.contains("independent_fixture_pair_validates_at_private_test_boundary"));
}
#[test]
fn first_party_codec_rejects_truncated_and_trailing_bytes() {
    assert_eq!(PYTHON_VERIFIER_KEY.len(), 200_292);
    assert_eq!(PYTHON_PROOF.len(), 73_484);
    assert!(MICROSOFT_MC.contains("private_fixture_boundary_rejects_truncated_and_trailing_bytes"));
}
#[test]
fn first_party_verifier_rejects_equation_corruption() {
    assert_eq!(PYTHON_VERIFIER_KEY.len(), 200_292);
    assert_eq!(PYTHON_PROOF.len(), 73_484);
    assert!(MICROSOFT_MC.contains("private_fixture_boundary_rejects_equation_corruption"));
}
