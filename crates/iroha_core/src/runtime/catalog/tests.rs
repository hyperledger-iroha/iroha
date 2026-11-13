//! The release catalog is closed, complete and pinned.

use std::{collections::BTreeSet, fmt::Write as _};

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
    write!(
        out,
        "  \"selection\": \"{}\",\n  \"definitions\": [\n",
        catalog.baseline_id
    )
    .expect("writing to a String cannot fail");
    for (index, (_, kind, name, id)) in rows.iter().enumerate() {
        let comma = if index + 1 == rows.len() { "" } else { "," };
        writeln!(
            out,
            "    {{\"kind\": \"{kind}\", \"name\": {}, \"id\": \"{id}\"}}{comma}",
            norito::json::to_json(name).expect("a name is JSON")
        )
        .expect("writing to a String cannot fail");
    }
    out.push_str("  ]\n}\n");
    out
}

#[test]
fn the_baseline_is_closed_and_binds_every_foundation_instruction() {
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
        .filter(|wire_id| is_baseline_instruction(wire_id))
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

pub(crate) fn privacy_resolution_instructions() -> (
    iroha_data_model::isi::InstructionBox,
    iroha_data_model::isi::InstructionBox,
    iroha_data_model::isi::InstructionBox,
) {
    use iroha_data_model::{
        isi::privacy::{
            BootstrapPrivacyIronwoodPoolV1, SubmitPrivacyIronwoodProofV1, SubmitPrivacyProofV1,
        },
        privacy::{
            PrivacyActionRoleV1, PrivacyIronwoodActionsStatementV1, PrivacyIronwoodPoolBootstrapV1,
            PrivacyIronwoodProofEnvelopeV1, PrivacyProofEnvelopeV1, PrivacyStatementV1,
            privacy_action_role_samples_v1,
        },
    };
    let old: PrivacyProofEnvelopeV1 = privacy_action_role_samples_v1()
        .unwrap()
        .into_iter()
        .find(|sample| sample.role == PrivacyActionRoleV1::OrchardActions)
        .unwrap()
        .envelope
        .try_into()
        .unwrap();
    let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = old.statement.clone() else {
        unreachable!()
    };
    let bootstrap = PrivacyIronwoodPoolBootstrapV1::new(
        statement.pool_id,
        statement.asset_definition_id.clone(),
        statement.public_balance_scope,
        iroha_test_samples::ALICE_ID.clone(),
    )
    .unwrap();
    let new = PrivacyIronwoodProofEnvelopeV1::bind_v1(
        PrivacyIronwoodActionsStatementV1::from(statement),
        old.proof.bytes().clone(),
    )
    .unwrap();
    (
        SubmitPrivacyProofV1::new(old).into(),
        SubmitPrivacyIronwoodProofV1::new(new).into(),
        BootstrapPrivacyIronwoodPoolV1::new(bootstrap).into(),
    )
}

#[test]
fn ironwood_storage_transition_is_required_for_every_native_admission() {
    use iroha_data_model::{
        isi::{
            InstructionBox,
            error::{InstructionExecutionError, InvalidParameterError},
            privacy::SchedulePrivacyIronwoodLimitsUpdateV1,
        },
        privacy::{OrchardLimitsV1, PrivacyProtocolIdV1, PrivacyRuntimeProtocolIdV1},
        runtime::lifecycle::RuntimeRejectionV1,
    };
    let catalog = release_catalog().unwrap();
    assert!(catalog.definitions.len() > catalog.baseline_definitions.len());
    let successor = &catalog.ironwood.as_ref().unwrap().selection;
    successor.check_closure(&catalog.definitions).unwrap();
    let (old, new, bootstrap) = privacy_resolution_instructions();
    assert_eq!(
        crate::runtime::resolve(&old, &catalog.baseline, 1, || None),
        Ok(Some(catalog.baseline.bindings[0].implementation))
    );
    let limits: InstructionBox = SchedulePrivacyIronwoodLimitsUpdateV1::new(
        2,
        OrchardLimitsV1 {
            max_action_count: 2,
        },
    )
    .into();
    for instruction in [&new, &bootstrap, &limits] {
        let wire_id = iroha_data_model::isi::instruction_wire_id(instruction).unwrap();
        assert_eq!(
            crate::runtime::resolve(instruction, &catalog.baseline, 1, || None),
            Err(InstructionExecutionError::Runtime(
                RuntimeRejectionV1::DefinitionInactive {
                    wire_id: wire_id.into()
                },
            )),
            "each additive wire is absent from the baseline"
        );
        let mut forged = catalog.baseline.clone();
        let mut binding = forged.bindings[0].clone();
        binding.wire_id = wire_id.into();
        forged.bindings.retain(|old| old.wire_id != wire_id);
        forged.bindings.push(binding);
        forged.bindings.sort_by(|a, b| a.wire_id.cmp(&b.wire_id));
        assert_eq!(
            crate::runtime::resolve(instruction, &forged, 1, || None),
            Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(format!(
                    "Ironwood protocol storage is inactive for {wire_id}; the committed selection does not admit ironwood-halo2-actions-v1",
                )),
            )),
            "binding a known native handler cannot authorize successor storage"
        );
    }
    for protocol in PrivacyProtocolIdV1::ALL {
        assert_eq!(
            crate::runtime::ensure_privacy_protocol_storage_v1(
                protocol.into(),
                &catalog.baseline,
                iroha_data_model::isi::privacy::SubmitPrivacyProofV1::WIRE_ID
            ),
            Ok(())
        );
    }
    assert!(
        crate::runtime::ensure_privacy_protocol_storage_v1(
            PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
            &catalog.baseline,
            iroha_data_model::isi::privacy::SubmitPrivacyIronwoodProofV1::WIRE_ID,
        )
        .is_err()
    );
    for instruction in [&old, &new, &bootstrap, &limits] {
        crate::runtime::resolve(instruction, successor, 1, || None).unwrap();
    }
}

#[test]
fn every_canonical_world_field_has_a_snapshot_member_a_selection_can_leave_out() {
    let mut fields = Vec::new();
    flatten(WORLD_FIELDS, &mut fields);
    let empty = norito::json::to_json(&crate::state::World::default()).expect("empty World");
    let world = crate::state::deserialize::json_object_members(&empty).expect("World object");
    let mut beside_the_world = Vec::new();
    for field in fields {
        let canonical = matches!(
            field.role,
            Role::Canonical(Canonical::Table { .. } | Canonical::Cell(_))
        );
        let member = crate::runtime::world_snapshot_member(field.id);
        assert_eq!(member.is_some(), canonical, "{}", field.id);
        let Some((top, child)) = member else {
            continue;
        };
        let Some((_, raw)) = world.iter().find(|(key, _)| *key == top) else {
            beside_the_world.push(field.id);
            continue;
        };
        if let Some(child) = child {
            assert_eq!(child, field.member);
            assert!(
                crate::state::deserialize::json_object_members(raw)
                    .expect("a nested owner is one object")
                    .iter()
                    .any(|(key, _)| *key == child),
                "{} is written as member {child} of {top}",
                field.id
            );
        } else {
            assert_eq!(top, field.member);
        }
    }
    // The State snapshot writes some baseline fields beside the World object. No selection can
    // leave one of those out: `uninstalled_world_members` refuses a selection that does not
    // install it, a release that retires one first moves its writer into the World object, and
    // every field an addition adds is a member of the World object.
    assert!(beside_the_world.contains(&"world.space_directory_manifests"));
    let catalog = release_catalog().expect("release catalog");
    for addition in crate::runtime::catalog::additions::ADDITIONS {
        for entry in addition.storage {
            assert!(
                !beside_the_world.contains(&entry.field()),
                "{} adds {}, which no snapshot can leave out",
                addition.module,
                entry.field()
            );
        }
    }
    let mut without = catalog.baseline.clone();
    let storage = catalog.world_storage["world.space_directory_manifests"];
    without.installed_storage.retain(|id| *id != storage);
    assert!(
        crate::runtime::uninstalled_world_members(&without)
            .is_err_and(|reason| reason.contains("world.space_directory_manifests"))
    );
}

#[test]
fn every_nested_world_owner_reads_snapshots_through_the_selection() {
    let owners: Vec<&str> = WORLD_FIELDS
        .iter()
        .filter(|field| matches!(field.role, Role::Canonical(Canonical::Owner(_))))
        .map(|field| field.member)
        .collect();
    assert_eq!(owners, crate::runtime::SELECTION_AWARE_OWNERS);
    for field in WORLD_FIELDS {
        let Role::Canonical(Canonical::Owner(children)) = field.role else {
            continue;
        };
        assert!(
            children
                .iter()
                .all(|child| !matches!(child.role, Role::Canonical(Canonical::Owner(_)))),
            "{} nests another owner, which a snapshot cannot leave a member out of",
            field.id
        );
    }
}

#[test]
fn an_addition_adds_a_field_inside_a_nested_world_owner() {
    use super::additions::test_additions::{NESTED, NESTED_MEMBER, NESTED_TABLE};
    let catalog = ReleaseCatalog::build_from(&[NESTED]).expect("a nested field is added storage");
    let storage = catalog.world_storage[NESTED_TABLE];
    assert!(
        catalog
            .baseline
            .installed_storage
            .binary_search(&storage)
            .is_err(),
        "the baseline leaves the added nested field uninstalled"
    );
    assert_eq!(catalog.additions[0].storage[NESTED_TABLE], storage);
    super::test_catalog::with_additions(&[NESTED], || {
        let catalog = release_catalog().expect("test catalog");
        let before = crate::runtime::uninstalled_world_members(&catalog.baseline).unwrap();
        assert!(before.members.is_empty());
        assert_eq!(before.nested, [("triggers", vec![NESTED_MEMBER])]);
        assert_eq!(before.nested_in("triggers"), [NESTED_MEMBER]);
        assert!(before.nested_in("accounts").is_empty());
        let network = iroha_data_model::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([3; 32])),
        );
        let change = catalog
            .activation(
                NESTED.module,
                &catalog.baseline,
                &catalog.definitions,
                network,
                9,
                [0; 32],
            )
            .expect("draft the nested addition");
        let after = change
            .apply(&catalog.baseline, &catalog.definitions)
            .expect("the activation applies")
            .selection;
        assert!(
            crate::runtime::uninstalled_world_members(&after)
                .unwrap()
                .is_empty(),
            "installing the nested field brings it back into snapshots"
        );
    });
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
        let ironwood = catalog.ironwood.as_ref().unwrap();
        expected.extend(ironwood.definitions.iter().copied());
        let mut composed = catalog.clone();
        for predecessor in [&catalog.baseline, &ironwood.selection] {
            let selected = composed.lifecycle_successor(predecessor).unwrap();
            expected.insert(selected.services_profile);
            expected.extend(selected.active);
        }
        // Restricted-read delegation and the kernel's lifecycle State compile one composition
        // per predecessor family they support; each is published, never selected.
        for composition in catalog
            .restricted_read_compositions
            .values()
            .chain(catalog.runtime_lifecycle_compositions.values())
        {
            expected.extend(composition.definitions());
        }
        for composition in catalog.deferred_consent_compositions.values() {
            expected.insert(composition.services);
            expected.extend(composition.replacements.values().copied());
            expected.extend(composition.addition.definitions());
        }
        // Native escrow custody compiles one services profile per supported predecessor profile,
        // with its replacement implementations and opening interfaces; each is published, never
        // selected.
        for composition in catalog.native_escrow_capture_compositions.values() {
            expected.insert(composition.services);
            expected.extend(composition.replacements.values().copied());
            expected.extend(composition.addition.definitions());
        }
        for composition in catalog.native_amx_capture_compositions.values() {
            expected.insert(composition.services);
            expected.extend(composition.replacements.values().copied());
            expected.extend(composition.addition.definitions());
        }
        for composition in catalog.native_game_capture_compositions.values() {
            expected.insert(composition.services);
            expected.extend(composition.replacements.values().copied());
            expected.extend(composition.addition.definitions());
            for prerequisite in &composition.prerequisites {
                expected.extend(prerequisite.definitions());
            }
        }
        for composition in catalog.retirement_compositions.values() {
            expected.insert(composition.services);
            expected.extend(composition.replacements.values().copied());
            expected.extend(composition.addition.definitions());
        }
        for composition in catalog.contract_lifecycle_compositions.values() {
            expected.extend([composition.host, composition.registry]);
            expected.extend(composition.addition.definitions());
        }
        // The typed committed parameters are published, never part of a founded selection.
        expected.extend(catalog.parameter_definitions());
        // The keyed State profile publishes storage for the State-level owners it adopts, its
        // adoption kernel, its commitment profile and the State-row permission; a governed
        // change set selects them.
        expected.extend(catalog.keyed_storage.iter().copied());
        for descriptor in [
            iroha_data_model::runtime::keyed_state::adoption_kernel(&catalog.keyed_storage),
            crate::runtime::keyed_state::descriptor(),
            crate::runtime::state_row_permission::descriptor(),
        ] {
            expected.insert(descriptor.id().expect("keyed State identity"));
        }
        // The content-publishing permission is published alone; a governed change set selects it.
        expected.insert(
            crate::runtime::content_publish_permission::descriptor()
                .id()
                .expect("content-publishing permission identity"),
        );
        // The authority service adds only its exact compiled profile and native owners
        // composed with that profile; its declared interfaces/storage are counted above.
        for (id, descriptor) in &catalog.definitions {
            let profile = match descriptor {
                DefinitionDescriptorV1::ServicesProfile(_) => *id,
                DefinitionDescriptorV1::Implementation(implementation) => {
                    implementation.services_profile
                }
                _ => continue,
            };
            if catalog.service_enabled(
                profile,
                crate::runtime::sorafs_authority::SERVICE,
                iroha_crypto::Hash::new(crate::runtime::sorafs_authority::SERVICE_CLOSURE).into(),
            ) == Ok(true)
            {
                expected.insert(*id);
            }
        }
        assert_eq!(
            catalog.definitions.keys().copied().collect::<BTreeSet<_>>(),
            expected,
            "every definition belongs to the baseline or an explicitly compiled addition"
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
                added.contains(instruction.wire_id)
                    || !is_baseline_instruction(instruction.wire_id),
                "{} must be registered tolerantly exactly when an addition adds it",
                instruction.wire_id
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
        // The same release without the addition: it compiles the field the addition claims as
        // baseline storage, under another definition.
        let missing = build(&[])
            .unsupported(&applied.selection)
            .expect("a release without the addition stops before it activates");
        assert!(
            addition.definitions().contains(&missing),
            "it names one of the addition's definitions, not {missing}: {:?}",
            catalog.definitions.get(&missing)
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
                "a derived member of a nested owner",
                vec![addition(
                    "iroha.test.a",
                    &[LOG_WIRE_ID],
                    &[],
                    &[StorageAddition::Table("triggers.ids")],
                )],
            ),
            (
                "a nested owner itself",
                vec![addition(
                    "iroha.test.a",
                    &[LOG_WIRE_ID],
                    &[],
                    &[StorageAddition::Table("world.triggers")],
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

#[test]
fn lifecycle_successor_preserves_baseline_and_requires_committed_services() {
    let catalog = release_catalog().unwrap();
    catalog
        .lifecycle
        .check_closure(&catalog.definitions)
        .unwrap();
    assert_ne!(catalog.lifecycle_id, catalog.baseline_id);
    // Count the standalone lifecycle delta, not unrelated compiled Ironwood
    // definitions. Its builder must preserve every frozen baseline preimage and
    // add exactly the reviewed services profile and native bundle.
    let mut lifecycle_only = catalog.clone();
    lifecycle_only
        .definitions
        .retain(|id, _| catalog.baseline_definitions.contains(id));
    let frozen_baseline = lifecycle_only.definitions.clone();
    let successor = lifecycle_only
        .lifecycle_successor(&catalog.baseline)
        .unwrap();
    assert_eq!(successor, catalog.lifecycle);
    let expected_added = BTreeSet::from([
        catalog.lifecycle.services_profile,
        catalog.lifecycle.bindings[0].implementation,
    ]);
    assert_eq!(expected_added.len(), 2);
    assert_eq!(
        lifecycle_only.definitions.len(),
        frozen_baseline.len() + expected_added.len()
    );
    assert_eq!(
        lifecycle_only
            .definitions
            .keys()
            .filter(|id| !frozen_baseline.contains_key(*id))
            .copied()
            .collect::<BTreeSet<_>>(),
        expected_added
    );
    for (id, descriptor) in &frozen_baseline {
        assert_eq!(lifecycle_only.definitions.get(id), Some(descriptor));
    }
    assert!(
        !catalog
            .baseline_definitions
            .contains(&catalog.lifecycle.services_profile)
    );
    assert!(catalog.unsupported(&catalog.lifecycle).is_none());
    assert_eq!(inventory(catalog), std::fs::read_to_string(PINNED).unwrap());
    assert!(!super::super::trigger_lifecycle_enabled(&catalog.baseline).unwrap());
    assert!(super::super::trigger_lifecycle_enabled(&catalog.lifecycle).unwrap());
}

#[test]
fn lifecycle_services_reject_unknown_or_incompletely_rebound_selection() {
    let catalog = release_catalog().unwrap();
    let mut selection = catalog.baseline.clone();
    selection.services_profile = DefinitionId([0xa3; 32]);
    assert!(super::super::trigger_lifecycle_enabled(&selection).is_err());
    selection.services_profile = catalog.lifecycle.services_profile;
    assert!(selection.check_closure(&catalog.definitions).is_err());
}

#[test]
fn lifecycle_successor_composes_with_current_ironwood_owners_in_both_orders() {
    use super::super::{ironwood::activation_change_set_for_selection, lifecycle_activation};

    let catalog = release_catalog().unwrap();
    let baseline = catalog.baseline.clone();
    let baseline_published: BTreeMap<_, _> = catalog
        .baseline_definitions
        .iter()
        .map(|id| (*id, catalog.definitions[id].clone()))
        .collect();
    let network = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0x22; 32])),
    );

    // Publish only what the actual Ironwood transition adds to the committed
    // baseline. Being compiled into this release does not publish future rows.
    let ironwood_change = activation_change_set_for_selection(
        network,
        99,
        [0x32; 32],
        &baseline,
        &baseline_published,
    )
    .unwrap();
    let installed = ironwood_change
        .apply(&baseline, &baseline_published)
        .unwrap();
    let ironwood = installed.selection;
    assert_eq!(ironwood, catalog.ironwood.as_ref().unwrap().selection);
    let mut published = baseline_published.clone();
    published.extend(installed.published);
    ironwood.check_closure(&published).unwrap();

    let mut first = catalog.clone();
    let compiled = first.definitions.clone();
    first.compile_lifecycle_compositions().unwrap();
    let composed = first.lifecycle_successor(&ironwood).unwrap();
    assert_eq!(
        first.definitions, compiled,
        "composition must be compiled before governance proposes it"
    );
    assert_eq!(composed.installed_storage, ironwood.installed_storage);
    assert_eq!(
        composed
            .bindings
            .iter()
            .map(|b| (&b.wire_id, b.interface))
            .collect::<Vec<_>>(),
        ironwood
            .bindings
            .iter()
            .map(|b| (&b.wire_id, b.interface))
            .collect::<Vec<_>>()
    );
    assert_ne!(composed.installed_storage, baseline.installed_storage);
    let change = lifecycle_activation::from_selection(
        first.clone(),
        &ironwood,
        &published,
        network,
        100,
        [0x33; 32],
    )
    .unwrap();
    assert_eq!(change.predecessor, ironwood.id().unwrap());
    assert_eq!(
        change
            .publish
            .iter()
            .map(|descriptor| descriptor.id().unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            composed.services_profile,
            composed.bindings[0].implementation,
        ]),
        "the actual network still needs the exact compiled lifecycle pair published"
    );
    let applied = change.apply(&ironwood, &published).unwrap();
    assert_eq!(applied.selection, composed);
    assert_eq!(
        applied.selection.installed_storage,
        ironwood.installed_storage
    );
    published.extend(applied.published);
    composed.check_closure(&published).unwrap();

    // Reverse order uses the same production constructors and its own actual
    // publication history, never the complete compiled inventory as network State.
    let reverse = catalog.clone();
    let lifecycle_change = lifecycle_activation::from_selection(
        reverse.clone(),
        &baseline,
        &baseline_published,
        network,
        99,
        [0x34; 32],
    )
    .unwrap();
    let lifecycle_applied = lifecycle_change
        .apply(&baseline, &baseline_published)
        .unwrap();
    let lifecycle = lifecycle_applied.selection;
    assert_eq!(lifecycle, catalog.lifecycle);
    let mut reverse_published = baseline_published.clone();
    reverse_published.extend(lifecycle_applied.published);
    lifecycle.check_closure(&reverse_published).unwrap();
    let reverse_change = activation_change_set_for_selection(
        network,
        100,
        [0x35; 32],
        &lifecycle,
        &reverse_published,
    )
    .unwrap();
    assert_eq!(reverse_change.predecessor, lifecycle.id().unwrap());
    let reverse_applied = reverse_change
        .apply(&lifecycle, &reverse_published)
        .unwrap();
    let reverse_composed = reverse_applied.selection;
    reverse_published.extend(reverse_applied.published);
    reverse_composed.check_closure(&reverse_published).unwrap();
    assert_eq!(composed, reverse_composed);
    assert!(first.unsupported(&composed).is_none());
    assert!(reverse.unsupported(&reverse_composed).is_none());
    assert_eq!(reverse.definitions, compiled);
    for (id, descriptor) in &baseline_published {
        assert_eq!(published.get(id), Some(descriptor));
        assert_eq!(reverse_published.get(id), Some(descriptor));
    }
}

#[test]
fn lifecycle_successor_preserves_retained_restrictions_and_consumes_retired_bundle_restriction() {
    use iroha_data_model::runtime::definition::{RestrictionEntryV1, RestrictionV1};
    let mut catalog = release_catalog().unwrap().clone();
    let mut current = catalog.baseline.clone();
    let old = current.bindings[0].implementation;
    let interface = current.bindings[0].interface;
    current.restrictions = vec![
        RestrictionEntryV1 {
            definition: old,
            restriction: RestrictionV1::AdmissionClosed(2),
        },
        RestrictionEntryV1 {
            definition: interface,
            restriction: RestrictionV1::AdmissionClosed(3),
        },
    ];
    current.restrictions.sort_by_key(|entry| entry.definition);
    let successor = catalog.lifecycle_successor(&current).unwrap();
    assert_eq!(
        successor.restrictions,
        vec![RestrictionEntryV1 {
            definition: interface,
            restriction: RestrictionV1::AdmissionClosed(3)
        }]
    );
    let change = super::super::lifecycle_activation::from_selection(
        catalog.clone(),
        &current,
        &catalog.definitions,
        iroha_data_model::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0x22; 32])),
        ),
        100,
        [0x33; 32],
    )
    .unwrap();
    assert_eq!(
        change
            .apply(&current, &catalog.definitions)
            .unwrap()
            .selection,
        successor
    );
}
