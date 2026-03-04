//! Regression guard for the public AXT descriptor/manifest fixtures.

use hex::encode;
use iroha_data_model::nexus::{
    AxtDescriptor, AxtDescriptorBuilder, AxtTouchFragment, DataSpaceId, TouchManifest,
    compute_descriptor_binding, validate_descriptor,
};
use norito::{decode_from_bytes, json};

#[derive(Debug, Clone, norito::json::JsonDeserialize)]
struct DescriptorFixture {
    descriptor: AxtDescriptor,
    touch_manifest: Vec<AxtTouchFragment>,
    binding_hex: String,
    descriptor_hex: String,
}

#[test]
fn descriptor_fixture_and_binding_are_stable() {
    let fixture: DescriptorFixture =
        json::from_slice(include_bytes!("fixtures/axt_descriptor_multi_ds.json"))
            .expect("fixture decodes");

    validate_descriptor(&fixture.descriptor).expect("fixture descriptor is valid");

    // Builder should produce the same descriptor with deterministic ordering and deduplication.
    let built = AxtDescriptorBuilder::new()
        .dataspace(DataSpaceId::new(7))
        .dataspace(DataSpaceId::new(1))
        .touch(
            DataSpaceId::new(1),
            ["payments/", "orders/", "orders/"],
            ["ledger/"],
        )
        .touch(
            DataSpaceId::new(7),
            ["reports/"],
            ["audits/", "aggregates/", "audits/"],
        )
        .build()
        .expect("builder output valid");
    assert_eq!(built, fixture.descriptor);

    // Touch manifest schema should remain aligned with the descriptor dataspace set.
    for fragment in &fixture.touch_manifest {
        assert!(
            fixture.descriptor.dsids.contains(&fragment.dsid),
            "touch manifest for undeclared dataspace {}",
            fragment.dsid.as_u64()
        );
    }

    // Provide a small manifest sanity check to guard key ordering/deduplication.
    let expected_manifest = TouchManifest::from_read_write(
        ["reports/monthly", "reports/monthly"],
        ["aggregates/monthly", "audits/summary"],
    );
    assert_eq!(
        fixture
            .touch_manifest
            .iter()
            .find(|fragment| fragment.dsid == DataSpaceId::new(7))
            .map(|fragment| &fragment.manifest),
        Some(&expected_manifest)
    );

    let binding = compute_descriptor_binding(&fixture.descriptor).expect("binding computed");
    assert_eq!(encode(binding), fixture.binding_hex.to_lowercase());

    // Ensure descriptor bytes round-trip via Norito encoding.
    let encoded = norito::to_bytes(&fixture.descriptor).expect("descriptor encodes");
    assert_eq!(
        encode(&encoded),
        fixture.descriptor_hex.to_lowercase(),
        "descriptor encoding drifted"
    );
    let decoded: AxtDescriptor =
        decode_from_bytes(&encoded).expect("descriptor bytes decode deterministically");
    assert_eq!(decoded, fixture.descriptor);
}
