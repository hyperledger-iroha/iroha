//! Ensure signatory/quorum instructions are in the default registry.
use iroha_data_model::isi::registry;
use iroha_data_model::isi::{AddSignatory, RemoveSignatory, SetAccountQuorum};
use std::any::type_name;
#[test]
fn default_registry_includes_signatory_quorum_instructions() {
    let registry = registry::default();
    let type_names = [
        type_name::<AddSignatory>(),
        type_name::<RemoveSignatory>(),
        type_name::<SetAccountQuorum>(),
    ];
    for type_name in type_names {
        let wire_id = registry
            .wire_id(type_name)
            .expect("default registry has an explicit wire identifier");
        assert!(
            registry.decode(wire_id, &[]).is_some(),
            "default registry should include {type_name}"
        );
        assert!(registry.decode(type_name, &[]).is_none());
    }
}
