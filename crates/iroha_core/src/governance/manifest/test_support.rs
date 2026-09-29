//! Materialized native manifest fixtures using the ordinary bounded source loader.

use super::*;
use iroha_data_model::nexus::NativeLaneValidatorBindingV1;

/// Freeze exact validator manifests for the supplied active catalog.
/// The returned registry retains its parsed source after the temporary files disappear.
pub(crate) fn validator_registry(
    catalog: &LaneCatalog,
    governance: &GovernanceCatalog,
    bindings_by_lane: BTreeMap<LaneId, Vec<ManifestValidatorBinding>>,
) -> LaneManifestRegistry {
    let directory = tempfile::tempdir().expect("isolated native manifest fixture directory");
    for (lane_id, bindings) in &bindings_by_lane {
        let lane = catalog
            .lanes()
            .iter()
            .find(|lane| lane.id == *lane_id)
            .expect("manifest fixture lane must exist in the active catalog");
        let manifest = NativeLaneManifestV1 {
            version: Some(NativeLaneManifestV1::VERSION),
            lane: Some(lane.alias.clone()),
            governance: lane.governance.clone(),
            validators: Some(
                bindings
                    .iter()
                    .map(|binding| NativeLaneValidatorBindingV1 {
                        validator: Some(binding.validator.to_string()),
                        peer_id: Some(binding.peer_id.to_string()),
                        torii_url: binding.torii_url.clone(),
                    })
                    .collect(),
            ),
            ..NativeLaneManifestV1::default()
        };
        fs::write(
            directory
                .path()
                .join(format!("{}.manifest.json", lane.alias)),
            json::to_vec(&manifest).expect("encode native lane manifest fixture"),
        )
        .expect("write native lane manifest fixture");
    }
    let registry = LaneManifestRegistry::from_config(
        catalog,
        governance,
        &LaneRegistry {
            manifest_directory: Some(directory.path().to_path_buf()),
            ..LaneRegistry::default()
        },
    );
    directory
        .close()
        .expect("remove frozen manifest fixture files");
    registry
        .validate_materialized_authority_for_catalog(catalog, governance)
        .expect("fixture authority must retain the exact frozen source and catalog");
    for (lane_id, bindings) in bindings_by_lane {
        assert_eq!(
            registry.lane_validator_bindings(lane_id),
            Some(bindings),
            "the ordinary loader must preserve each declared validator binding"
        );
    }
    registry
}

#[test]
fn validator_registry_retains_exact_authority_after_source_removal() {
    let catalog = LaneCatalog::default();
    let governance = GovernanceCatalog::default();
    let validator = iroha_test_samples::ALICE_ID.clone();
    let binding = ManifestValidatorBinding {
        peer_id: PeerId::new(validator.expect_single_signatory().clone()),
        validator,
        torii_url: Some("https://validator.example".to_owned()),
    };
    let registry = validator_registry(
        &catalog,
        &governance,
        BTreeMap::from([(LaneId::SINGLE, vec![binding.clone()])]),
    );
    assert!(
        !registry
            .status(LaneId::SINGLE)
            .unwrap()
            .manifest_path
            .as_ref()
            .unwrap()
            .exists()
    );
    let rebound = registry.rebind(&catalog, &governance);
    assert!(registry.has_same_authority_as(&rebound));
    assert_eq!(
        rebound.lane_validator_bindings(LaneId::SINGLE),
        Some(vec![binding])
    );
}
