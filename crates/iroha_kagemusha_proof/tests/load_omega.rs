//! Genuine four-stage C4 Load wrapped by the complete Omega predicate.
//! Bootstrap-only or two-terminal catalogs remain explicit component scopes;
//! neither establishes the complete operation catalog or production byte gate.

/// Shared genuine Bootstrap predecessor and four-stage Load continuation fixtures.
#[path = "a_load_recursive.rs"]
pub mod load_chain;

include!("common/proof_fixtures/load_omega_body.rs");

#[test]
#[ignore = "real authenticated Bootstrap and four-stage Load with complete outer proof; run optimized"]
fn four_bus_authenticated_load_reaches_complete_outer_predicate() {
    let _ = diagnostic_load_omega(true);
}

#[test]
#[ignore = "real two-terminal four-bus source catalog rebuilt under one immutable Omega key"]
fn common_bootstrap_load_catalog_rebinds_every_proof_and_key() {
    let output = two_terminal_load_omega(true);
    assert_eq!(
        output.artifact.source.state.lineage[17],
        output
            .artifact
            .key
            .kagemusha_digest(&output.artifact.binding)
            .unwrap()
    );
}
