use hex::encode;
use iroha_data_model::nexus::DataSpaceId;
use ivm::axt::{AxtDescriptor, TouchManifest, compute_binding};
use norito::json::{self, Value};

#[derive(Debug, Clone, json::Serialize, json::Deserialize)]
struct FixtureManifest {
    dsid: DataSpaceId,
    manifest: TouchManifest,
}

#[derive(Debug, Clone, json::Serialize, json::Deserialize)]
struct DescriptorFixture {
    descriptor: AxtDescriptor,
    touch_manifests: Vec<FixtureManifest>,
    binding_hex: String,
}

const FIXTURE: &str = include_str!("fixtures/axt_descriptor_multi_ds.json");

#[test]
fn builder_matches_fixture_and_binding() {
    let fixture: DescriptorFixture =
        json::from_str(FIXTURE).expect("fixture must parse as Norito JSON");

    let (descriptor, binding) = AxtDescriptor::builder()
        // Intentionally shuffled inputs; builder must canonicalize ordering.
        .touch(
            DataSpaceId::new(7),
            ["manifests/", "proofs/", "da/"],
            ["proofs/"],
        )
        .touch(
            DataSpaceId::new(1),
            ["reports/agg/", "accounts/"],
            ["reports/out/"],
        )
        .build_with_binding()
        .expect("descriptor should be valid");

    assert_eq!(descriptor, fixture.descriptor);

    let binding_hex = encode(binding);
    assert_eq!(binding_hex, fixture.binding_hex);
}

#[test]
fn descriptor_fixture_roundtrips() {
    let fixture: DescriptorFixture =
        json::from_str(FIXTURE).expect("fixture must parse as Norito JSON");
    let fixture_value: Value = json::from_str(FIXTURE).expect("fixture value");

    let descriptor_value = json::to_value(&fixture.descriptor).expect("descriptor value");
    assert_eq!(
        descriptor_value,
        fixture_value
            .get("descriptor")
            .cloned()
            .expect("descriptor field missing"),
        "descriptor JSON layout drifted; update fixture if intentional",
    );

    let binding = compute_binding(&fixture.descriptor).expect("compute binding");
    let binding_hex = encode(binding);
    assert_eq!(
        binding_hex, fixture.binding_hex,
        "descriptor binding changed; refresh fixture intentionally if this is expected"
    );

    let manifest_value = json::to_value(&fixture.touch_manifests).expect("manifest value");
    assert_eq!(
        manifest_value,
        fixture_value
            .get("touch_manifests")
            .cloned()
            .expect("touch_manifests missing"),
        "touch manifest schema drifted; refresh fixture intentionally if this is expected"
    );
}
