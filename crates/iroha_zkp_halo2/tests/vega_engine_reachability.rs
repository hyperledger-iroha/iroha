//! Source-level release guard for the public Vega proof boundary.
#[path = "vega_microsoft_cross_conformance.rs"]
mod vega_microsoft_cross_conformance;
const ENGINE_SOURCE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega/engine.rs"));
const FACADE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega.rs"));
const EXACT_BOUNDARY_SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/vega/canonical_mc_exact.rs"
));
#[test]
fn production_vega_entry_points_delegate_only_to_canonical_mc() {
    let prove = source_between(
        ENGINE_SOURCE,
        "pub fn prove_vega_mdl_figure9_v1",
        "/// Verify one bounded",
    );
    assert_eq!(
        prove
            .matches("super::canonical_mc::prove_figure9_mc")
            .count(),
        1
    );
    let verify = source_between(
        ENGINE_SOURCE,
        "pub fn verify_vega_mdl_figure9_v1",
        "/// Number of uniform",
    );
    assert_eq!(
        verify
            .matches("super::canonical_mc::verify_figure9_mc")
            .count(),
        1
    );
    for retired in [
        "prove_masked_relaxed",
        "verify_masked_relaxed",
        "MaskedRelaxedProofWire",
        "VegaMdlProofWire",
        "super::spartan",
        "super::nifs",
        "super::hyrax",
    ] {
        assert!(
            !prove.contains(retired) && !verify.contains(retired),
            "retired custom Vega path is reachable through the public engine: {retired}"
        );
    }
}
#[test]
fn retired_custom_transcript_and_wire_are_not_public_facade_exports() {
    for retired_export in ["pub use transcript::", "pub use wire::", "pub use curve::"] {
        assert!(
            !FACADE_SOURCE.contains(retired_export),
            "retired custom Vega helper remains public: {retired_export}"
        );
    }
    assert!(FACADE_SOURCE.contains("pub use engine::{"));
    assert!(FACADE_SOURCE.contains("prove_vega_mdl_figure9_v1"));
    assert!(FACADE_SOURCE.contains("verify_vega_mdl_figure9_v1"));
}
#[test]
fn public_contract_matches_the_reachable_first_party_split_prover() {
    for stale_claim in [
        "split-witness adapter remains unavailable",
        "does not yet have the exact Microsoft split step/core witness adapter",
    ] {
        assert!(
            !ENGINE_SOURCE.contains(stale_claim),
            "public Vega contract regressed to a stale prover claim: {stale_claim}"
        );
    }
    let prove = source_between(
        EXACT_BOUNDARY_SOURCE,
        "pub(super) fn prove_figure9_mc",
        "/// Parse the fixed envelope",
    );
    for required in [
        "preflight_governed_figure9_prover_artifacts",
        "synthesize_figure9_mc_material",
        "prepare_governed_figure9_application",
        "verify_figure9_mc",
    ] {
        assert!(
            prove.contains(required),
            "exact Figure 9 prover omitted required stage: {required}"
        );
    }
}
fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let after_start = source
        .split_once(start)
        .unwrap_or_else(|| panic!("missing source marker: {start}"))
        .1;
    after_start
        .split_once(end)
        .unwrap_or_else(|| panic!("missing source marker: {end}"))
        .0
}
