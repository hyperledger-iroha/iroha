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

/// The baseline's identities and names in kind, name and identity order, as pinned. Additions
/// after the baseline are not pinned here: they change with each release until activated.
fn inventory(catalog: &ReleaseCatalog) -> String {
    let mut rows: Vec<(u8, String, String, String)> = catalog
        .baseline_definitions
        .iter()
        .map(|id| (id, &catalog.definitions[id]))
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
    let added: BTreeSet<&str> = catalog
        .additions
        .iter()
        .flat_map(|addition| addition.interfaces.keys().copied())
        .collect();
    let wire_ids: BTreeSet<&str> = iroha_data_model::isi::registry::built_in_instruction_schemas()
        .map(|instruction| instruction.wire_id)
        .filter(|wire_id| !added.contains(wire_id))
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
fn every_canonical_world_field_is_founded_or_added_storage() {
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
    let added: BTreeSet<&DefinitionId> = catalog
        .additions
        .iter()
        .flat_map(|addition| addition.storage.values())
        .collect();
    for (field, storage) in &catalog.world_storage {
        if added.contains(storage) {
            assert!(
                !installed.contains(storage),
                "{field} is added, not founded"
            );
            continue;
        }
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

mod addition_tests {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        NetworkId,
        runtime::{definition::BindingV1, lifecycle::SelectionTransitionV1},
    };

    use super::*;
    use crate::runtime::catalog::additions::{
        Addition, StorageAddition,
        test_additions::{CELL, CELL_INITIAL, LOG, LOG_MODULE, LOG_WIRE_ID, REBIND_LOG, TABLE},
    };

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x21; 32])))
    }

    fn build(additions: &[Addition]) -> ReleaseCatalog {
        ReleaseCatalog::build_from(additions).expect("catalog")
    }

    #[test]
    fn the_release_catalog_is_its_baseline_plus_its_additions() {
        let catalog = release_catalog().expect("release catalog");
        let rebuilt = build(crate::runtime::catalog::additions::ADDITIONS);
        assert_eq!(rebuilt.baseline, catalog.baseline);
        assert_eq!(rebuilt.definitions, catalog.definitions);
        assert_eq!(rebuilt.additions, catalog.additions);
        let mut expected: BTreeSet<DefinitionId> =
            catalog.baseline_definitions.iter().copied().collect();
        for addition in &catalog.additions {
            expected.extend(addition.definitions());
        }
        assert_eq!(
            catalog.definitions.keys().copied().collect::<BTreeSet<_>>(),
            expected,
            "every definition is the baseline's or an addition's"
        );
    }

    #[test]
    fn exactly_the_added_instructions_decode_tolerantly() {
        // A release that knows an added instruction must keep a frame its decoder refuses as
        // opaque, exactly as a release without it does (§5.2); baseline instructions, known to
        // every release of the foundation, decode strictly.
        let catalog = release_catalog().expect("release catalog");
        let added: BTreeSet<&str> = catalog
            .additions
            .iter()
            .flat_map(|addition| addition.interfaces.keys().copied())
            .collect();
        let registry = iroha_data_model::isi::registry::default();
        for instruction in iroha_data_model::isi::registry::built_in_instruction_schemas() {
            assert_eq!(
                registry.is_tolerant(instruction.wire_id),
                added.contains(instruction.wire_id),
                "{} must be registered tolerantly exactly when an addition adds it",
                instruction.wire_id
            );
        }
    }

    #[test]
    fn no_release_adds_a_world_field_before_snapshots_follow_the_selection() {
        for addition in crate::runtime::catalog::additions::ADDITIONS {
            assert!(
                addition.storage.is_empty(),
                "addition {} adds World fields; first make snapshot writers omit and readers \
                 default uninstalled fields (TODO at `additions::ADDITIONS`)",
                addition.module
            );
        }
    }

    #[test]
    fn an_addition_leaves_the_baseline_and_publishes_its_own_definitions() {
        let real = release_catalog().expect("release catalog");
        let catalog = build(&[LOG]);
        let addition = &catalog.additions[0];
        assert_eq!(addition.module, LOG_MODULE);
        assert_eq!(
            catalog.native_module(&addition.implementation),
            Some(LOG_MODULE)
        );
        assert_eq!(
            catalog.native_module(&catalog.baseline.bindings[0].implementation),
            Some(BASELINE_MODULE)
        );

        assert!(
            catalog
                .baseline
                .bindings
                .iter()
                .all(|binding| binding.wire_id != LOG_WIRE_ID),
            "the baseline no longer binds what the addition claims"
        );
        let table = catalog.world_storage[TABLE];
        let cell = catalog.world_storage[CELL];
        assert_eq!(
            addition.storage,
            BTreeMap::from([(CELL, cell), (TABLE, table)])
        );
        for storage in [table, cell] {
            assert!(
                catalog
                    .baseline
                    .installed_storage
                    .binary_search(&storage)
                    .is_err()
            );
            assert!(!catalog.baseline_definitions.contains(&storage));
        }
        let Some(DefinitionDescriptorV1::Storage(descriptor)) = catalog.definitions.get(&cell)
        else {
            panic!("the cell is a storage definition");
        };
        assert_eq!(
            descriptor.initial.as_deref(),
            Some(norito::encode_canonical(&CELL_INITIAL).unwrap().as_slice()),
            "an added cell publishes its initial value"
        );
        assert!(catalog.cell_installer(&cell).is_some());
        assert!(catalog.cell_installer(&table).is_none());
        assert_ne!(
            Some(&table),
            real.world_storage.get(TABLE),
            "the addition's storage contract differs from a founded field's"
        );

        let interface = addition.interfaces[LOG_WIRE_ID];
        let Some(DefinitionDescriptorV1::Implementation(implementation)) =
            catalog.definitions.get(&addition.implementation)
        else {
            panic!("the addition publishes an implementation");
        };
        assert_eq!(
            implementation.entrypoint,
            EntrypointV1::Instructions(vec![interface])
        );
        for storage in catalog
            .baseline
            .installed_storage
            .iter()
            .chain([&table, &cell])
        {
            assert!(implementation.writes.binary_search(storage).is_ok());
        }
        for binding in &catalog.baseline.bindings {
            assert!(
                implementation
                    .invokes
                    .binary_search(&binding.interface)
                    .is_ok()
            );
        }
        assert!(implementation.invokes.binary_search(&interface).is_ok());

        for id in [
            catalog.baseline.kernel,
            catalog.baseline.commitment_profile,
            catalog.baseline.services_profile,
        ] {
            assert_eq!(
                real.definitions.get(&id),
                catalog.definitions.get(&id),
                "claims never touch the kernel or the profiles"
            );
        }
    }

    #[test]
    fn activation_moves_a_root_to_the_addition() {
        let catalog = build(&[LOG]);
        let addition = &catalog.additions[0];
        let published = catalog
            .baseline_definitions
            .iter()
            .map(|id| (*id, catalog.definitions[id].clone()))
            .collect::<BTreeMap<_, _>>();
        let change = catalog
            .activation(
                LOG_MODULE,
                &catalog.baseline,
                &published,
                network(),
                50,
                [9; 32],
            )
            .expect("activation");
        assert_eq!(change.predecessor, catalog.baseline_id);
        assert_eq!(
            change
                .publish
                .iter()
                .map(|descriptor| descriptor.id().unwrap())
                .collect::<Vec<_>>(),
            addition.definitions()
        );
        assert!(
            change
                .transitions
                .contains(&SelectionTransitionV1::InstallStorage(
                    addition.storage[CELL]
                ))
        );
        let applied = change
            .apply(&catalog.baseline, &published)
            .expect("the change set applies");
        assert_eq!(applied.selection.id().unwrap(), change.successor);
        let bound = applied
            .selection
            .bindings
            .iter()
            .find(|binding| binding.wire_id == LOG_WIRE_ID)
            .expect("the addition binds its wire ID");
        assert_eq!(
            bound,
            &BindingV1 {
                wire_id: LOG_WIRE_ID.to_owned(),
                interface: addition.interfaces[LOG_WIRE_ID],
                implementation: addition.implementation,
            }
        );
        assert_eq!(catalog.unsupported(&applied.selection), None);
        let missing = release_catalog()
            .expect("release catalog")
            .unsupported(&applied.selection)
            .expect("a release without the addition stops before it activates");
        assert!(
            addition.definitions().contains(&missing),
            "it names one of the addition's definitions"
        );

        let mut after = published.clone();
        after.extend(applied.published);
        assert!(
            catalog
                .activation(
                    LOG_MODULE,
                    &applied.selection,
                    &after,
                    network(),
                    60,
                    [9; 32]
                )
                .is_err(),
            "an active addition is not activated twice"
        );
        assert!(
            catalog
                .activation(
                    "iroha.unknown",
                    &catalog.baseline,
                    &published,
                    network(),
                    50,
                    [9; 32]
                )
                .is_err()
        );
    }

    #[test]
    fn a_rebinding_addition_follows_the_addition_it_rebinds() {
        let catalog = build(&[LOG, REBIND_LOG]);
        let first = &catalog.additions[0];
        let second = &catalog.additions[1];
        assert_eq!(
            second.rebinds,
            BTreeMap::from([(LOG_WIRE_ID, first.interfaces[LOG_WIRE_ID])])
        );
        let mut published = catalog
            .baseline_definitions
            .iter()
            .map(|id| (*id, catalog.definitions[id].clone()))
            .collect::<BTreeMap<_, _>>();
        assert!(
            catalog
                .activation(
                    second.module,
                    &catalog.baseline,
                    &published,
                    network(),
                    50,
                    [9; 32]
                )
                .is_err(),
            "additions activate in order"
        );
        let change = catalog
            .activation(
                LOG_MODULE,
                &catalog.baseline,
                &published,
                network(),
                50,
                [9; 32],
            )
            .expect("first addition");
        let applied = change.apply(&catalog.baseline, &published).unwrap();
        published.extend(applied.published);
        let change = catalog
            .activation(
                second.module,
                &applied.selection,
                &published,
                network(),
                60,
                [9; 32],
            )
            .expect("second addition");
        let rebound = change
            .apply(&applied.selection, &published)
            .unwrap()
            .selection;
        assert_eq!(
            rebound
                .bindings
                .iter()
                .find(|binding| binding.wire_id == LOG_WIRE_ID)
                .map(|binding| binding.implementation),
            Some(second.implementation)
        );
    }

    #[test]
    fn malformed_additions_do_not_build() {
        const fn addition(
            module: &'static str,
            instructions: &'static [&'static str],
            rebinds: &'static [&'static str],
            storage: &'static [StorageAddition],
        ) -> Addition {
            Addition {
                module,
                instructions,
                rebinds,
                storage,
            }
        }
        for (case, additions) in [
            (
                "baseline module",
                vec![addition(BASELINE_MODULE, &[LOG_WIRE_ID], &[], &[])],
            ),
            (
                "repeated module",
                vec![
                    addition("iroha.test.a", &[LOG_WIRE_ID], &[], &[]),
                    addition("iroha.test.a", &["iroha.register"], &[], &[]),
                ],
            ),
            (
                "answers nothing",
                vec![addition("iroha.test.a", &[], &[], &[])],
            ),
            (
                "claimed twice",
                vec![
                    addition("iroha.test.a", &[LOG_WIRE_ID], &[], &[]),
                    addition("iroha.test.b", &[LOG_WIRE_ID], &[], &[]),
                ],
            ),
            (
                "not compiled",
                vec![addition("iroha.test.a", &["iroha.test.missing"], &[], &[])],
            ),
            (
                "not a canonical field",
                vec![addition(
                    "iroha.test.a",
                    &[LOG_WIRE_ID],
                    &[],
                    &[StorageAddition::Table("world.test.missing")],
                )],
            ),
            (
                "wrong shape",
                vec![addition(
                    "iroha.test.a",
                    &[LOG_WIRE_ID],
                    &[],
                    &[StorageAddition::Table(CELL)],
                )],
            ),
            (
                "rebinds what nothing before it answers",
                vec![addition("iroha.test.a", &[], &[LOG_WIRE_ID], &[]), LOG],
            ),
        ] {
            assert!(
                matches!(
                    ReleaseCatalog::build_from(&additions),
                    Err(CatalogError::Addition { .. })
                ),
                "{case}"
            );
        }
        let rebinds_baseline = addition("iroha.test.fix", &[], &["iroha.register"], &[]);
        let catalog = build(&[rebinds_baseline]);
        assert!(
            catalog
                .baseline
                .bindings
                .iter()
                .any(|binding| binding.wire_id == "iroha.register"),
            "rebinding a baseline wire ID leaves the baseline intact"
        );
    }
}
