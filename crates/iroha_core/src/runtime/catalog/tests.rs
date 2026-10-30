//! The release catalog is closed, complete and pinned.

use std::collections::BTreeSet;

use iroha_data_model::runtime::definition::{
    DefinitionDescriptorV1, EntrypointV1, ImplementationCodeV1,
};

use super::*;

const PINNED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../specs/runtime_baseline_catalog.json"
);

/// The catalog's identities and names in kind, name and identity order, as pinned.
fn inventory(catalog: &ReleaseCatalog) -> String {
    let mut rows: Vec<(u8, String, String, String)> = catalog
        .definitions
        .iter()
        .map(|(id, descriptor)| {
            let name = match descriptor {
                DefinitionDescriptorV1::Interface(interface) => interface.wire_id.clone(),
                DefinitionDescriptorV1::Implementation(implementation) => {
                    match &implementation.code {
                        ImplementationCodeV1::Native { module, .. } => module.clone(),
                        ImplementationCodeV1::Ivm { entrypoint, .. } => entrypoint.clone(),
                    }
                }
                DefinitionDescriptorV1::Storage(storage) => storage.namespace.clone(),
                DefinitionDescriptorV1::Event(event) => event.name.clone(),
                DefinitionDescriptorV1::Permission(permission) => permission.name.clone(),
                DefinitionDescriptorV1::Parameter(parameter) => parameter.name.clone(),
                DefinitionDescriptorV1::HostProfile(host) => format!("abi.v{}", host.abi_version),
                DefinitionDescriptorV1::ServicesProfile(_) => "services".to_owned(),
                DefinitionDescriptorV1::CommitmentProfile(profile) => profile.scheme.clone(),
                DefinitionDescriptorV1::Kernel(kernel) => kernel.name.clone(),
                DefinitionDescriptorV1::Migration(_) => "migration".to_owned(),
            };
            let kind = descriptor.kind();
            (kind.tag(), format!("{kind:?}"), name, id.to_string())
        })
        .collect();
    rows.sort();
    let mut out = String::from("{\n");
    out.push_str(
        "  \"note\": \"Baseline definitions of the live-upgrade foundation (specs/live_runtime_upgrades.md §3.1). Frozen once a running network installs them; regenerate only before that with `cargo test -p iroha_core --lib regenerate_runtime_baseline_catalog -- --ignored`.\",\n",
    );
    out.push_str(&format!(
        "  \"selection\": \"{}\",\n  \"definitions\": [\n",
        catalog.baseline_id
    ));
    for (index, (_, kind, name, id)) in rows.iter().enumerate() {
        let comma = if index + 1 == rows.len() { "" } else { "," };
        out.push_str(&format!(
            "    {{\"kind\": \"{kind}\", \"name\": {}, \"id\": \"{id}\"}}{comma}\n",
            norito::json::to_json(name).expect("a name is JSON")
        ));
    }
    out.push_str("  ]\n}\n");
    out
}

#[test]
fn the_baseline_is_closed_and_binds_every_built_in_instruction() {
    let catalog = release_catalog().expect("release catalog");
    catalog
        .baseline
        .check_closure(&catalog.definitions)
        .expect("closed baseline");

    let bundle = catalog.baseline.bindings[0].implementation;
    let wire_ids: BTreeSet<&str> = iroha_data_model::isi::registry::built_in_instruction_schemas()
        .map(|instruction| instruction.wire_id)
        .collect();
    let bound: BTreeSet<&str> = catalog
        .baseline
        .bindings
        .iter()
        .map(|binding| {
            assert_eq!(binding.implementation, bundle, "{}", binding.wire_id);
            binding.wire_id.as_str()
        })
        .collect();
    assert_eq!(bound, wire_ids);

    let Some(DefinitionDescriptorV1::Implementation(implementation)) =
        catalog.definitions.get(&bundle)
    else {
        panic!("the bundle is an implementation");
    };
    let EntrypointV1::Instructions(answered) = &implementation.entrypoint else {
        panic!("the bundle answers instructions");
    };
    assert_eq!(answered.len(), wire_ids.len());
    assert!(matches!(
        &implementation.code,
        ImplementationCodeV1::Native { module, .. } if module == BASELINE_MODULE
    ));
}

#[test]
fn every_canonical_world_field_is_installed_storage() {
    let catalog = release_catalog().expect("release catalog");
    let mut fields = Vec::new();
    flatten(WORLD_FIELDS, &mut fields);
    let canonical: BTreeSet<&str> = fields
        .iter()
        .filter(|field| {
            matches!(
                field.role,
                Role::Canonical(Canonical::Table { .. } | Canonical::Cell(_))
            )
        })
        .map(|field| field.id)
        .collect();
    assert_eq!(
        catalog
            .world_storage
            .keys()
            .copied()
            .collect::<BTreeSet<_>>(),
        canonical
    );
    let installed: BTreeSet<&DefinitionId> = catalog.baseline.installed_storage.iter().collect();
    for (field, storage) in &catalog.world_storage {
        assert!(installed.contains(storage), "{field} is not installed");
        let Some(DefinitionDescriptorV1::Storage(descriptor)) = catalog.definitions.get(storage)
        else {
            panic!("{field} maps to a storage definition");
        };
        assert_eq!(descriptor.namespace, *field);
        assert_eq!(
            descriptor.initial, None,
            "{field} is founded, not initialized"
        );
    }
}

#[test]
fn the_release_supports_its_baseline() {
    let catalog = release_catalog().expect("release catalog");
    assert_eq!(catalog.unsupported(&catalog.baseline), None);

    let mut foreign = catalog.baseline.clone();
    foreign.active.push(DefinitionId([0xff; 32]));
    assert_eq!(
        catalog.unsupported(&foreign),
        Some(DefinitionId([0xff; 32]))
    );
}

#[test]
fn the_baseline_matches_its_pinned_inventory() {
    let catalog = release_catalog().expect("release catalog");
    let expected = std::fs::read_to_string(PINNED).expect("pinned baseline inventory");
    let actual = inventory(catalog);
    if expected != actual {
        let first = expected
            .lines()
            .zip(actual.lines())
            .position(|(expected, actual)| expected != actual)
            .unwrap_or_else(|| expected.lines().count().min(actual.lines().count()));
        panic!(
            "the baseline catalog differs from specs/runtime_baseline_catalog.json from line {}:\n  pinned: {}\n  built:  {}\nA network that installed the baseline can never execute a different one: publish the change as a new definition. Before any network installs it, regenerate with `cargo test -p iroha_core --lib regenerate_runtime_baseline_catalog -- --ignored`.",
            first + 1,
            expected.lines().nth(first).unwrap_or("<end>"),
            actual.lines().nth(first).unwrap_or("<end>"),
        );
    }
}

#[test]
#[ignore = "rewrites specs/runtime_baseline_catalog.json"]
fn regenerate_runtime_baseline_catalog() {
    let catalog = release_catalog().expect("release catalog");
    std::fs::write(PINNED, inventory(catalog)).expect("write the pinned inventory");
}
