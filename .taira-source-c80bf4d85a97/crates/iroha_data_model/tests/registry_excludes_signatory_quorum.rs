//! Ensure signatory/quorum instructions are in the default registry.

use std::any::type_name;

use iroha_data_model::isi::registry;
use iroha_data_model::isi::{AddSignatory, RemoveSignatory, SetAccountQuorum};

#[test]
fn default_registry_includes_signatory_quorum_instructions() {
    let registry = registry::default();
    let names = [
        type_name::<AddSignatory>(),
        type_name::<RemoveSignatory>(),
        type_name::<SetAccountQuorum>(),
    ];

    for name in names {
        assert!(
            registry.decode(name, &[]).is_some(),
            "default registry should include {name}"
        );
    }
}
