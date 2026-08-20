//! Source-level release guard for the public Vega proof boundary.
#[path = "vega_microsoft_cross_conformance.rs"]
mod vega_microsoft_cross_conformance;
const ENGINE_SOURCE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega/engine.rs"));
const FACADE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega.rs"));
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
