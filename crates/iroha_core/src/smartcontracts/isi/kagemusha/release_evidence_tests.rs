//! Current standalone release evidence used by registry and runtime custody tests.

use iroha_data_model::kagemusha::{
    KagemushaGovernedVerifierRegistryV1, KagemushaInternalValidationReceiptV1,
    KagemushaReleaseAttestationV1, KagemushaReleaseManifestV1,
};

/// Decode current canonical release objects with public fixture signatures.
/// These fixtures establish no finalized network or approved release authority.
pub(crate) fn release_evidence() -> (
    KagemushaGovernedVerifierRegistryV1,
    KagemushaReleaseManifestV1,
    KagemushaInternalValidationReceiptV1,
    KagemushaReleaseAttestationV1,
) {
    let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/kagemusha/governed_release_evidence_v1.json"
    )))
    .expect("standalone current release evidence");
    (
        norito::json::from_value(fixture["registry"].clone()).unwrap(),
        norito::json::from_value(fixture["manifest"].clone()).unwrap(),
        norito::json::from_value(fixture["receipt"].clone()).unwrap(),
        norito::json::from_value(fixture["attestation"].clone()).unwrap(),
    )
}

#[test]
fn standalone_evidence_authenticates_exact_release_and_rejects_changed_receipt() {
    let (mut registry, manifest, receipt, attestation) = release_evidence();
    let predecessor = registry.clone();
    registry
        .install_authenticated_release(&manifest, &receipt, &attestation)
        .expect("exact threshold-authenticated standby");
    assert_eq!(registry.releases.len(), 1);
    assert_eq!(registry.releases[0].release_id, manifest.release_id);
    let frame = norito::encode_canonical(&registry).unwrap();
    assert_eq!(
        norito::decode_canonical::<KagemushaGovernedVerifierRegistryV1>(&frame).unwrap(),
        registry
    );
    let mut changed_receipt = receipt;
    changed_receipt.source_tree_digest[0] ^= 1;
    assert!(
        predecessor
            .clone()
            .install_authenticated_release(&manifest, &changed_receipt, &attestation)
            .is_err()
    );
}
