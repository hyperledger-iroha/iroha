//! Canonical public topology instruction schema, without runtime authority capabilities.
use super::{IntoSchema, find_missing_schema_references};
use iroha_data_model::isi::sorafs::MutateSorafsTopologyAuthority;

#[test]
fn topology_instruction_exports_its_complete_typed_v1_schema() {
    let expected = MutateSorafsTopologyAuthority::schema();
    let exported = crate::build_schemas();
    let exported: std::collections::BTreeMap<_, _> = exported.iter().collect();
    for (id, descriptor) in expected.iter() {
        assert_eq!(
            exported.get(id).copied(),
            Some(descriptor),
            "missing or substituted topology descriptor: {}",
            descriptor.type_name
        );
    }
    assert!(find_missing_schema_references(&expected).is_empty());
    for name in [
        "MutateSorafsTopologyAuthority",
        "TopologyTransitionV1",
        "TopologyActionV1",
    ] {
        assert_eq!(
            expected
                .iter()
                .filter(|(_, entry)| entry.type_name == name)
                .count(),
            1,
            "canonical topology descriptor occurs once: {name}"
        );
    }
}
