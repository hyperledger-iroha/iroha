//! Exact current catalog mutation and historical physical storage schemas.

use super::{IntoSchema, Metadata, find_missing_schema_references};
use iroha_data_model::nexus::{
    LaneLifecycleStatusV1, NexusCatalogTransitionV1, NexusRuntimeCatalogV1,
    RuntimeDataSpaceRetirementRecordV1, RuntimeDataSpaceRetirementV1, RuntimeLaneRetirementV1,
};

#[test]
fn catalog_export_contains_native_retirement_and_historical_storage_closure() {
    let mut expected = NexusCatalogTransitionV1::schema();
    NexusRuntimeCatalogV1::update_schema_map(&mut expected);
    LaneLifecycleStatusV1::update_schema_map(&mut expected);
    let exported = crate::build_schemas();
    let entries: std::collections::BTreeMap<_, _> = exported.iter().collect();
    for (id, descriptor) in expected.iter() {
        assert_eq!(
            entries.get(id).copied(),
            Some(descriptor),
            "missing or substituted native catalog descriptor: {}",
            descriptor.type_name,
        );
    }
    assert!(find_missing_schema_references(&expected).is_empty());
    assert!(exported.contains_key::<RuntimeDataSpaceRetirementV1>());
    assert!(exported.contains_key::<RuntimeDataSpaceRetirementRecordV1>());
    assert!(exported.contains_key::<RuntimeLaneRetirementV1>());
    let Metadata::Struct(transition) = exported.get::<NexusCatalogTransitionV1>().unwrap() else {
        panic!("current catalog transition must remain a typed struct");
    };
    assert_eq!(
        transition
            .declarations
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        [
            "version",
            "expected_catalog_hash",
            "expected_incarnation_root",
            "expected_runtime_catalog_hash",
            "dataspace_additions",
            "lane_additions",
            "manifest_additions",
            "dataspace_retirements",
            "lane_retirements",
        ],
    );
    let Metadata::Struct(runtime) = exported.get::<NexusRuntimeCatalogV1>().unwrap() else {
        panic!("current catalog history must remain a typed struct");
    };
    assert!(
        runtime
            .declarations
            .iter()
            .any(|field| field.name == "retired_dataspaces")
    );
    assert!(
        runtime
            .declarations
            .iter()
            .any(|field| field.name == "retired_lanes")
    );
}
