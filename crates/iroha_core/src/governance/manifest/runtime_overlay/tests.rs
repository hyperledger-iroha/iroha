//! Deterministic additive manifest derivation and immutable baseline regression tests.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::nexus::{DataSpaceMetadata, LaneLifecyclePlan, NativeLaneValidatorBindingV1};
use iroha_primitives::json::Json;
use nonzero_ext::nonzero;

fn fixture() -> (
    LaneManifestRegistry,
    LaneCatalog,
    DataSpaceCatalog,
    GovernanceCatalog,
) {
    let baseline_catalog = LaneCatalog::new(
        nonzero!(2_u32),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: LaneId::new(1),
                alias: "existing".to_owned(),
                ..LaneConfig::default()
            },
        ],
    )
    .expect("baseline catalog");
    let governance = GovernanceCatalog::default();
    let baseline =
        LaneManifestRegistry::from_config(&baseline_catalog, &governance, &LaneRegistry::default());
    let effective = baseline_catalog
        .apply_lifecycle(&LaneLifecyclePlan {
            additions: vec![LaneConfig {
                id: LaneId::new(5),
                alias: "bpng".to_owned(),
                dataspace_id: DataSpaceId::new(42),
                ..LaneConfig::default()
            }],
            retire: vec![],
        })
        .expect("additive lane catalog");
    let dataspaces = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: DataSpaceId::new(42),
            alias: "bpng".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("physical dataspace catalog");
    (baseline, effective, dataspaces, governance)
}

#[test]
fn manifest_catalog_binding_rejects_stale_refresh_after_source_preserving_lifecycle() {
    let (baseline, catalog, _, governance) = fixture();
    let candidate = baseline.rebind(&catalog, &governance);
    let retired = catalog
        .apply_lifecycle(&LaneLifecyclePlan {
            additions: vec![],
            retire: vec![LaneId::new(1)],
        })
        .expect("ordinary manual lifecycle");
    let current = candidate.rebind(&retired, &governance);
    assert_eq!(
        candidate.consensus_policy_digest(),
        current.consensus_policy_digest()
    );
    assert_eq!(
        candidate.baseline_consensus_policy_digest(),
        current.baseline_consensus_policy_digest()
    );
    assert!(candidate.is_bound_to_catalog(&catalog));
    assert!(!candidate.is_bound_to_catalog(&retired));
    assert!(current.is_bound_to_catalog(&retired));
    assert!(!LaneManifestRegistry::empty().is_bound_to_catalog(&catalog));
    let status_only = LaneManifestRegistry::from_statuses(candidate.statuses.clone());
    assert!(!status_only.is_bound_to_catalog(&catalog));
    assert!(
        !status_only
            .rebind(&catalog, &governance)
            .is_bound_to_catalog(&catalog)
    );
}
