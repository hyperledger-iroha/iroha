//! Exact private-root registration and compact-anchor schemas are public protocol roots.
use super::{IntoSchema, find_missing_schema_references};
use iroha_data_model::{
    block::consensus::SumeragiRootScope,
    private_dataspace::{
        PrivateDataspaceAnchor, PrivateDataspaceAnchorState, PrivateDataspaceCursor,
        PrivateDataspaceRegistration,
    },
};

#[test]
fn private_dataspace_export_contains_complete_registered_schema_closure() {
    let mut expected = PrivateDataspaceAnchorState::schema();
    PrivateDataspaceAnchor::update_schema_map(&mut expected);
    iroha_data_model::private_dataspace::PrivateDataspaceRegistry::update_schema_map(&mut expected);
    iroha_data_model::private_dataspace::PrivateDataspaceAdmissionPolicy::update_schema_map(
        &mut expected,
    );
    iroha_data_model::isi::private_dataspace::RegisterPrivateDataspace::update_schema_map(
        &mut expected,
    );
    iroha_data_model::isi::private_dataspace::AnchorPrivateDataspace::update_schema_map(
        &mut expected,
    );
    iroha_data_model::smart_contract::ContractArtifactId::update_schema_map(&mut expected);
    let exported = crate::build_schemas();
    let exported: std::collections::BTreeMap<_, _> = exported.iter().collect();
    for (id, descriptor) in expected.iter() {
        assert_eq!(
            exported.get(id).copied(),
            Some(descriptor),
            "missing or substituted private-root descriptor: {}",
            descriptor.type_name,
        );
    }
    assert!(expected.contains_key::<SumeragiRootScope>());
    assert!(expected.contains_key::<PrivateDataspaceRegistration>());
    assert!(expected.contains_key::<PrivateDataspaceCursor>());
    assert!(find_missing_schema_references(&expected).is_empty());
}
