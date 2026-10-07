//! Real compact Bootstrap with its actual immutable outer key in every signed
//! object and source proof. The catalog contains one terminal only; broader
//! catalog, operation and performance qualification remain separate gates.

include!("proof_fixtures/compact_bootstrap_body.rs");

#[test]
#[ignore = "actual authenticated source chain rebuilt under its compact outer key; run optimized"]
fn compact_bootstrap_binds_its_actual_immutable_outer_key() {
    let artifact = rooted_compact_bootstrap();
    assert_eq!(artifact.proof.len() + 1088, 4800);
    assert_eq!(
        artifact.source.state.lineage[17],
        artifact.key.kagemusha_digest(&artifact.binding).unwrap(),
    );
}
