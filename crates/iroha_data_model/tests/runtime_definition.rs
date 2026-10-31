//! Live-upgrade definitions: identities, format rules and root-selection closure.

use std::collections::BTreeMap;

use iroha_data_model::runtime::definition::{
    BindingV1, CarrierFieldV1, CarrierNodeV1, CarrierSchemaV1, CarrierVariantV1,
    CommitmentProfileDescriptorV1, CompletionClosureV1, DefinitionDescriptorV1, DefinitionError,
    DefinitionId, DefinitionKind, EntrypointV1, EventDescriptorV1, ImplementationCodeV1,
    ImplementationDescriptorV1, InterfaceDescriptorV1, KernelDescriptorV1, PermissionDescriptorV1,
    ProjectionV1, ResourceLimitsV1, RestrictionEntryV1, RestrictionV1, RootSelectionV1, SchemaV1,
    SelectionError, ServiceEntryV1, ServicesProfileDescriptorV1, StorageDescriptorV1,
    StorageShapeV1,
};

const LIMITS: ResourceLimitsV1 = ResourceLimitsV1 {
    max_gas: 1_000_000,
    max_effects: 64,
    max_call_depth: 8,
};

fn codec(name: &str) -> SchemaV1 {
    SchemaV1::Codec(format!("norito:{name}@1.0.0"))
}

fn kernel() -> DefinitionDescriptorV1 {
    DefinitionDescriptorV1::Kernel(KernelDescriptorV1 {
        name: "iroha.kernel".into(),
        reference_closure: [1; 32],
    })
}

fn commitment() -> DefinitionDescriptorV1 {
    DefinitionDescriptorV1::CommitmentProfile(CommitmentProfileDescriptorV1 {
        scheme: "iroha.keyed_state.v1".into(),
        parameters: [2; 32],
    })
}

fn services() -> DefinitionDescriptorV1 {
    DefinitionDescriptorV1::ServicesProfile(ServicesProfileDescriptorV1 {
        services: vec![
            ServiceEntryV1 {
                name: "authorization".into(),
                reference_closure: [3; 32],
            },
            ServiceEntryV1 {
                name: "fees".into(),
                reference_closure: [4; 32],
            },
        ],
    })
}

fn bonds_table() -> DefinitionDescriptorV1 {
    DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
        namespace: "world.bonds".into(),
        shape: StorageShapeV1::Table {
            key: codec("iroha_data_model::asset::id::AssetDefinitionId"),
        },
        value: codec("iroha_data_model::bond::BondRecord"),
        initial: None,
        invariants: [5; 32],
        projection: ProjectionV1::BareNorito,
    })
}

fn bond_policy_cell() -> DefinitionDescriptorV1 {
    DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
        namespace: "world.bond_policy".into(),
        shape: StorageShapeV1::Cell,
        value: codec("u32"),
        initial: Some(vec![0, 0, 0, 0]),
        invariants: [6; 32],
        projection: ProjectionV1::BareNorito,
    })
}

fn bond_event() -> DefinitionDescriptorV1 {
    DefinitionDescriptorV1::Event(EventDescriptorV1 {
        name: "iroha.event.bond.v1".into(),
        schema: codec("iroha_data_model::events::data::BondEvent"),
    })
}

fn bond_permission() -> DefinitionDescriptorV1 {
    DefinitionDescriptorV1::Permission(PermissionDescriptorV1 {
        name: "CanManageBonds".into(),
        payload: codec("iroha_data_model::asset::id::AssetDefinitionId"),
    })
}

pub fn interface(wire_id: &str, event: DefinitionId) -> DefinitionDescriptorV1 {
    DefinitionDescriptorV1::Interface(InterfaceDescriptorV1 {
        wire_id: wire_id.into(),
        payload: codec("iroha_data_model::isi::bond::BondInstructionBox"),
        rejection_codes: vec!["bond_not_found".into(), "not_permitted".into()],
        events: vec![event],
        metering_class: "native.flat".into(),
        contract_statement: [7; 32],
    })
}

pub struct Ids {
    pub(crate) kernel: DefinitionId,
    pub(crate) commitment: DefinitionId,
    pub(crate) services: DefinitionId,
    pub(crate) table: DefinitionId,
    pub(crate) cell: DefinitionId,
    pub(crate) event: DefinitionId,
    pub(crate) permission: DefinitionId,
}

pub fn implementation(ids: &Ids, interface: DefinitionId, module: &str) -> DefinitionDescriptorV1 {
    DefinitionDescriptorV1::Implementation(ImplementationDescriptorV1 {
        entrypoint: EntrypointV1::Instructions(vec![interface]),
        code: ImplementationCodeV1::Native {
            module: module.into(),
            reference_closure: [8; 32],
        },
        reads: sorted(vec![ids.table, ids.cell]),
        writes: vec![ids.table],
        scans: Vec::new(),
        events: vec![ids.event],
        permissions: vec![ids.permission],
        parameters: Vec::new(),
        invokes: Vec::new(),
        host_profile: None,
        services_profile: ids.services,
        limits: LIMITS,
    })
}

pub fn sorted(mut ids: Vec<DefinitionId>) -> Vec<DefinitionId> {
    ids.sort();
    ids
}

pub fn publish(
    catalog: &mut BTreeMap<DefinitionId, DefinitionDescriptorV1>,
    descriptor: DefinitionDescriptorV1,
) -> DefinitionId {
    let id = descriptor.id().expect("valid descriptor");
    catalog.insert(id, descriptor);
    id
}

/// A catalog with one bond interface, its implementation and a valid selection.
pub fn bond_root() -> (
    BTreeMap<DefinitionId, DefinitionDescriptorV1>,
    Ids,
    RootSelectionV1,
) {
    let mut catalog = BTreeMap::new();
    let ids = Ids {
        kernel: publish(&mut catalog, kernel()),
        commitment: publish(&mut catalog, commitment()),
        services: publish(&mut catalog, services()),
        table: publish(&mut catalog, bonds_table()),
        cell: publish(&mut catalog, bond_policy_cell()),
        event: publish(&mut catalog, bond_event()),
        permission: publish(&mut catalog, bond_permission()),
    };
    let claim = publish(&mut catalog, interface("iroha.bond.claim.v1", ids.event));
    let claim_impl = publish(&mut catalog, implementation(&ids, claim, "bond.claim"));
    let selection = RootSelectionV1 {
        kernel: ids.kernel,
        commitment_profile: ids.commitment,
        services_profile: ids.services,
        installed_storage: sorted(vec![ids.table, ids.cell]),
        active: sorted(vec![ids.event, ids.permission, claim, claim_impl]),
        bindings: vec![BindingV1 {
            wire_id: "iroha.bond.claim.v1".into(),
            interface: claim,
            implementation: claim_impl,
        }],
        restrictions: Vec::new(),
        completion_closures: Vec::new(),
    };
    (catalog, ids, selection)
}

#[test]
fn definition_and_selection_ids_are_pinned() {
    let (catalog, ids, selection) = bond_root();
    let actual: Vec<(&str, String)> = vec![
        ("kernel", ids.kernel.to_string()),
        ("commitment", ids.commitment.to_string()),
        ("services", ids.services.to_string()),
        ("table", ids.table.to_string()),
        ("cell", ids.cell.to_string()),
        ("event", ids.event.to_string()),
        ("permission", ids.permission.to_string()),
        ("selection", selection.id().unwrap().to_string()),
    ];
    assert_eq!(catalog.len(), 9);
    let expected: Vec<(&str, String)> = PINNED
        .iter()
        .map(|(name, hex)| (*name, (*hex).to_owned()))
        .collect();
    assert_eq!(actual, expected, "identity vectors changed");
}

/// Identity vectors. A change here is a change of every definition and selection identity.
const PINNED: &[(&str, &str)] = &[
    (
        "kernel",
        "63524f70eca01053d91d07b5ade4ce0c2f3a403a826f2ac04fa3cc870d9a9abd",
    ),
    (
        "commitment",
        "9fd5e1296f80b6c1697cca38cd4bf2ac07c4af2c7b59d797904ac6466abf4109",
    ),
    (
        "services",
        "92819f1221289d722b3c2af00273438125d496e660e6cdfc4013a2c3b4b65f7b",
    ),
    (
        "table",
        "2b8664d3c15857347c65457622d848c3b693627f41dbb5ac38b3c27f1127baad",
    ),
    (
        "cell",
        "d9a9a1953e32a33573c06a5830aade28cc41b8c100e23db56d5a5bf8481aee91",
    ),
    (
        "event",
        "0e9b4e7901d2915c9c78a0801f401215d0f366e33a6cfb785aa0c38cb52eeeaf",
    ),
    (
        "permission",
        "b1ca98f11b0b943a89bd3b47fa3f8eb2ea92f5d643cb9cae75ca14a588447f33",
    ),
    (
        "selection",
        "0c09eadc951a364afad42dec152dea44761bd5f55a701caf0d2dbe7f836fa471",
    ),
];

#[test]
fn definition_id_binds_every_field() {
    let base = bonds_table();
    let base_id = base.id().unwrap();
    let mut changed = base.clone();
    if let DefinitionDescriptorV1::Storage(storage) = &mut changed {
        storage.invariants[0] ^= 1;
    }
    assert_ne!(changed.id().unwrap(), base_id);
    let mut renamed = base;
    if let DefinitionDescriptorV1::Storage(storage) = &mut renamed {
        storage.namespace = "world.bonds_v2".into();
    }
    assert_ne!(renamed.id().unwrap(), base_id);
}

#[test]
fn descriptors_round_trip_canonically() {
    let (catalog, _, selection) = bond_root();
    for descriptor in catalog.values() {
        let bytes = descriptor.canonical_bytes().unwrap();
        let decoded: DefinitionDescriptorV1 = norito::decode_canonical(&bytes).unwrap();
        assert_eq!(&decoded, descriptor);
    }
    let bytes = norito::encode_canonical(&selection).unwrap();
    let decoded: RootSelectionV1 = norito::decode_canonical(&bytes).unwrap();
    assert_eq!(decoded, selection);
}

#[test]
fn kinds_are_distinct_and_stable() {
    let tags: Vec<u8> = [
        DefinitionKind::Interface,
        DefinitionKind::Implementation,
        DefinitionKind::Storage,
        DefinitionKind::Event,
        DefinitionKind::Permission,
        DefinitionKind::Parameter,
        DefinitionKind::HostProfile,
        DefinitionKind::ServicesProfile,
        DefinitionKind::CommitmentProfile,
        DefinitionKind::Kernel,
        DefinitionKind::Migration,
    ]
    .iter()
    .map(|kind| kind.tag())
    .collect();
    assert_eq!(tags, (0..=10).collect::<Vec<u8>>());
}

#[test]
fn format_rules_reject_malformed_descriptors() {
    let (_, ids, _) = bond_root();
    let unsorted = DefinitionDescriptorV1::Interface(InterfaceDescriptorV1 {
        wire_id: "iroha.bond.claim.v1".into(),
        payload: codec("u8"),
        rejection_codes: vec!["z".into(), "a".into()],
        events: Vec::new(),
        metering_class: "native.flat".into(),
        contract_statement: [0; 32],
    });
    assert!(matches!(
        unsorted.validate(),
        Err(DefinitionError::NonCanonicalList { .. })
    ));

    let bad_wire = interface("iroha bond", ids.event);
    assert!(matches!(
        bad_wire.validate(),
        Err(DefinitionError::InvalidText { .. })
    ));

    let mut table_with_initial = bonds_table();
    if let DefinitionDescriptorV1::Storage(storage) = &mut table_with_initial {
        storage.initial = Some(vec![1]);
    }
    assert!(matches!(
        table_with_initial.validate(),
        Err(DefinitionError::InvalidInitial(_))
    ));

    let mut empty_cell = bond_policy_cell();
    if let DefinitionDescriptorV1::Storage(storage) = &mut empty_cell {
        storage.initial = Some(Vec::new());
    }
    assert!(matches!(
        empty_cell.validate(),
        Err(DefinitionError::InvalidInitial(_))
    ));

    let mut founded_cell = bond_policy_cell();
    if let DefinitionDescriptorV1::Storage(storage) = &mut founded_cell {
        storage.initial = None;
    }
    assert_eq!(
        founded_cell.validate(),
        Ok(()),
        "the act that founds a root populates a cell without initial bytes"
    );

    for entrypoint in [Vec::new(), vec![ids.event, ids.event]] {
        let mut bundle = implementation(&ids, ids.event, "bond.bundle");
        if let DefinitionDescriptorV1::Implementation(implementation) = &mut bundle {
            implementation.entrypoint = EntrypointV1::Instructions(entrypoint);
        }
        assert_eq!(
            bundle.validate(),
            Err(DefinitionError::NonCanonicalList {
                field: "entrypoint"
            })
        );
    }

    let mut native_on_host = implementation(&ids, ids.event, "bond.claim");
    if let DefinitionDescriptorV1::Implementation(implementation) = &mut native_on_host {
        implementation.host_profile = Some(ids.kernel);
    }
    assert!(matches!(
        native_on_host.validate(),
        Err(DefinitionError::HostProfileMismatch(_))
    ));

    let mut ivm_without_host = implementation(&ids, ids.event, "bond.claim");
    if let DefinitionDescriptorV1::Implementation(implementation) = &mut ivm_without_host {
        implementation.code = ImplementationCodeV1::Ivm {
            code_hash: [9; 32],
            entrypoint: "claim".into(),
        };
    }
    assert!(matches!(
        ivm_without_host.validate(),
        Err(DefinitionError::HostProfileMismatch(_))
    ));

    let mut zero_limits = implementation(&ids, ids.event, "bond.claim");
    if let DefinitionDescriptorV1::Implementation(implementation) = &mut zero_limits {
        implementation.limits.max_gas = 0;
    }
    assert_eq!(zero_limits.validate(), Err(DefinitionError::ZeroLimits));
    assert!(
        zero_limits.id().is_err(),
        "an invalid descriptor has no identity"
    );

    let bad_codec = DefinitionDescriptorV1::Event(EventDescriptorV1 {
        name: "iroha.event.x".into(),
        schema: SchemaV1::Codec("rust:Foo".into()),
    });
    assert!(matches!(
        bad_codec.validate(),
        Err(DefinitionError::InvalidText { .. })
    ));
}

fn carrier(nodes: Vec<CarrierNodeV1>) -> Result<(), DefinitionError> {
    CarrierSchemaV1 { nodes }.validate()
}

#[test]
fn carrier_schemas_are_bounded_and_ordered() {
    // A struct { owner: AccountId, amounts: Vec<u64> (<= 16), memo: Option<String (<= 64)> }.
    assert_eq!(
        carrier(vec![
            CarrierNodeV1::AccountId,
            CarrierNodeV1::U64,
            CarrierNodeV1::Vec {
                element: 1,
                max_len: 16,
            },
            CarrierNodeV1::String { max_bytes: 64 },
            CarrierNodeV1::Option { inner: 3 },
            CarrierNodeV1::Struct {
                fields: vec![
                    CarrierFieldV1 {
                        name: "owner".into(),
                        node: 0,
                    },
                    CarrierFieldV1 {
                        name: "amounts".into(),
                        node: 2,
                    },
                    CarrierFieldV1 {
                        name: "memo".into(),
                        node: 4,
                    },
                ],
            },
        ]),
        Ok(())
    );
    assert!(carrier(Vec::new()).is_err());
    assert!(
        carrier(vec![CarrierNodeV1::Option { inner: 0 }]).is_err(),
        "a node cannot refer to itself"
    );
    assert!(
        carrier(vec![CarrierNodeV1::Option { inner: 1 }, CarrierNodeV1::U8]).is_err(),
        "a node cannot refer forward"
    );
    assert!(carrier(vec![CarrierNodeV1::Bytes { max_bytes: 0 }]).is_err());
    assert!(
        carrier(vec![CarrierNodeV1::Bytes {
            max_bytes: (1 << 20) + 1
        }])
        .is_err()
    );
    assert!(
        carrier(vec![
            CarrierNodeV1::U8,
            CarrierNodeV1::Struct {
                fields: vec![
                    CarrierFieldV1 {
                        name: "a".into(),
                        node: 0,
                    },
                    CarrierFieldV1 {
                        name: "a".into(),
                        node: 0,
                    },
                ],
            },
        ])
        .is_err(),
        "duplicate field names"
    );
    assert!(
        carrier(vec![
            CarrierNodeV1::Unit,
            CarrierNodeV1::Enum {
                variants: vec![
                    CarrierVariantV1 {
                        discriminant: 1,
                        name: "b".into(),
                        node: 0,
                    },
                    CarrierVariantV1 {
                        discriminant: 1,
                        name: "c".into(),
                        node: 0,
                    },
                ],
            },
        ])
        .is_err(),
        "discriminants must strictly increase"
    );
    let mut deep = vec![CarrierNodeV1::U8];
    for index in 0..16 {
        deep.push(CarrierNodeV1::Option { inner: index });
    }
    assert!(carrier(deep).is_err(), "depth beyond the bound");
}

#[test]
fn a_valid_selection_satisfies_closure() {
    let (catalog, _, selection) = bond_root();
    assert_eq!(selection.check_closure(&catalog), Ok(()));
}

#[test]
fn adding_one_instruction_publishes_only_its_definitions() {
    let (mut catalog, ids, selection) = bond_root();
    let before: BTreeMap<DefinitionId, DefinitionDescriptorV1> = catalog.clone();
    let call = publish(&mut catalog, interface("iroha.bond.call.v1", ids.event));
    let call_impl = publish(&mut catalog, implementation(&ids, call, "bond.call"));

    let mut successor = selection.clone();
    successor.active = sorted([successor.active, vec![call, call_impl]].concat());
    successor.bindings.push(BindingV1 {
        wire_id: "iroha.bond.call.v1".into(),
        interface: call,
        implementation: call_impl,
    });
    successor
        .bindings
        .sort_by(|left, right| left.wire_id.cmp(&right.wire_id));

    assert_eq!(successor.check_closure(&catalog), Ok(()));
    assert_ne!(successor.id().unwrap(), selection.id().unwrap());
    for (id, descriptor) in &before {
        assert_eq!(
            descriptor.id().unwrap(),
            *id,
            "existing definitions keep their identity"
        );
    }
    assert_eq!(catalog.len(), before.len() + 2);
}

#[test]
fn closure_violations_are_refused() {
    let (catalog, ids, selection) = bond_root();
    let claim_impl = selection.bindings[0].implementation;
    let claim = selection
        .active
        .iter()
        .copied()
        .find(|id| matches!(catalog[id], DefinitionDescriptorV1::Interface(_)))
        .unwrap();

    let mut missing_storage = selection.clone();
    missing_storage.installed_storage = vec![ids.cell];
    assert_eq!(
        missing_storage.check_closure(&catalog),
        Err(SelectionError::MissingDependency {
            dependent: claim_impl,
            missing: ids.table,
        })
    );

    let mut unbound = selection.clone();
    unbound.bindings.clear();
    assert_eq!(
        unbound.check_closure(&catalog),
        Err(SelectionError::Unbound("iroha.bond.claim.v1".into()))
    );

    let mut wrong_target = selection.clone();
    wrong_target.bindings[0].implementation = claim;
    assert_eq!(
        wrong_target.check_closure(&catalog),
        Err(SelectionError::BadBinding("iroha.bond.claim.v1".into()))
    );

    let mut storage_activated = selection.clone();
    storage_activated.active = sorted([storage_activated.active, vec![ids.table]].concat());
    assert_eq!(
        storage_activated.check_closure(&catalog),
        Err(SelectionError::NotActivatable(ids.table))
    );

    let mut wrong_kernel = selection.clone();
    wrong_kernel.kernel = ids.table;
    assert_eq!(
        wrong_kernel.check_closure(&catalog),
        Err(SelectionError::WrongKind {
            id: ids.table,
            expected: DefinitionKind::Kernel,
        })
    );

    let mut unpublished = selection.clone();
    unpublished.commitment_profile = DefinitionId([0xee; 32]);
    assert_eq!(
        unpublished.check_closure(&catalog),
        Err(SelectionError::Unpublished(DefinitionId([0xee; 32])))
    );

    let mut unordered = selection.clone();
    unordered.active.reverse();
    assert_eq!(
        unordered.check_closure(&catalog),
        Err(SelectionError::NonCanonical("active"))
    );

    let mut outside = selection.clone();
    outside.restrictions = vec![RestrictionEntryV1 {
        definition: ids.kernel,
        restriction: RestrictionV1::Disabled(5),
    }];
    assert_eq!(
        outside.check_closure(&catalog),
        Err(SelectionError::Outside(ids.kernel))
    );

    let mut restricted = selection.clone();
    restricted.restrictions = vec![RestrictionEntryV1 {
        definition: claim_impl,
        restriction: RestrictionV1::AdmissionClosed(5),
    }];
    assert_eq!(restricted.check_closure(&catalog), Ok(()));
}

#[test]
fn completion_closures_keep_the_kernel_and_profiles_that_restrictions_cannot_target() {
    let (catalog, ids, selection) = bond_root();
    let claim_impl = selection.bindings[0].implementation;
    let closure = |definitions| CompletionClosureV1 {
        obligation: [0x51; 32],
        definitions: sorted(definitions),
    };

    let mut retained = selection.clone();
    retained.completion_closures = vec![closure(vec![
        ids.kernel,
        ids.commitment,
        ids.services,
        ids.table,
        claim_impl,
    ])];
    assert_eq!(
        retained.check_closure(&catalog),
        Ok(()),
        "a completion runs under the kernel and commits under the commitment profile"
    );

    let foreign = DefinitionId([0xee; 32]);
    let mut outside = selection.clone();
    outside.completion_closures = vec![closure(vec![ids.kernel, foreign])];
    assert_eq!(
        outside.check_closure(&catalog),
        Err(SelectionError::Outside(foreign))
    );

    for profile in [ids.kernel, ids.commitment, ids.services] {
        let mut restricted = selection.clone();
        restricted.restrictions = vec![RestrictionEntryV1 {
            definition: profile,
            restriction: RestrictionV1::AdmissionClosed(5),
        }];
        assert_eq!(
            restricted.check_closure(&catalog),
            Err(SelectionError::Outside(profile)),
            "the kernel and profiles are replaced, never restricted"
        );
    }

    let restricted = |restriction| {
        let mut restricted = retained.clone();
        restricted.restrictions = vec![RestrictionEntryV1 {
            definition: claim_impl,
            restriction,
        }];
        restricted.check_closure(&catalog)
    };
    assert_eq!(
        restricted(RestrictionV1::Disabled(5)),
        Err(SelectionError::ObligationStranded(claim_impl))
    );
    assert_eq!(
        restricted(RestrictionV1::CompletionOnly(5)),
        Ok(()),
        "completion-only still completes"
    );
}

#[test]
fn duplicate_wire_ids_and_foreign_services_are_refused() {
    let (mut catalog, ids, selection) = bond_root();
    let mut other_event = bond_event();
    if let DefinitionDescriptorV1::Event(event) = &mut other_event {
        event.name = "iroha.event.bond.v2".into();
    }
    let event_v2 = publish(&mut catalog, other_event);
    let twin = publish(&mut catalog, interface("iroha.bond.claim.v1", event_v2));
    let mut duplicate = selection.clone();
    duplicate.active = sorted([duplicate.active, vec![twin, event_v2]].concat());
    assert_eq!(
        duplicate.check_closure(&catalog),
        Err(SelectionError::DuplicateWireId(
            "iroha.bond.claim.v1".into()
        ))
    );

    let mut other_services = services();
    if let DefinitionDescriptorV1::ServicesProfile(profile) = &mut other_services {
        profile.services[0].reference_closure = [0xaa; 32];
    }
    let foreign = publish(&mut catalog, other_services);
    let foreign_ids = Ids {
        services: foreign,
        ..ids
    };
    let claim = selection
        .active
        .iter()
        .copied()
        .find(|id| matches!(catalog[id], DefinitionDescriptorV1::Interface(_)))
        .unwrap();
    let foreign_impl = publish(
        &mut catalog,
        implementation(&foreign_ids, claim, "bond.claim.foreign"),
    );
    let mut mixed = selection;
    mixed.active = sorted(
        [
            mixed
                .active
                .into_iter()
                .filter(|id| *id != mixed.bindings[0].implementation)
                .collect(),
            vec![foreign_impl],
        ]
        .concat(),
    );
    mixed.bindings[0].implementation = foreign_impl;
    assert_eq!(
        mixed.check_closure(&catalog),
        Err(SelectionError::ForeignServices(foreign_impl))
    );
}

#[test]
fn a_bundle_answers_every_interface_it_lists() {
    let (mut catalog, ids, mut selection) = bond_root();
    let claim = selection.bindings[0].implementation;
    let claim_interface = selection
        .active
        .iter()
        .copied()
        .find(|id| matches!(catalog[id], DefinitionDescriptorV1::Interface(_)))
        .expect("claim interface");
    let call = publish(&mut catalog, interface("iroha.bond.call.v1", ids.event));
    let mut bundle = implementation(&ids, claim_interface, "bond.bundle");
    if let DefinitionDescriptorV1::Implementation(implementation) = &mut bundle {
        implementation.entrypoint = EntrypointV1::Instructions(sorted(vec![claim_interface, call]));
    }
    let bundle = publish(&mut catalog, bundle);

    selection.active = sorted(
        selection
            .active
            .iter()
            .copied()
            .filter(|id| *id != claim)
            .chain([call, bundle])
            .collect(),
    );
    selection.bindings = vec![
        BindingV1 {
            wire_id: "iroha.bond.call.v1".into(),
            interface: call,
            implementation: bundle,
        },
        BindingV1 {
            wire_id: "iroha.bond.claim.v1".into(),
            interface: claim_interface,
            implementation: bundle,
        },
    ];
    assert_eq!(selection.check_closure(&catalog), Ok(()));

    let unlisted = publish(&mut catalog, interface("iroha.bond.repay.v1", ids.event));
    selection.active = sorted([selection.active, vec![unlisted]].concat());
    selection.bindings.push(BindingV1 {
        wire_id: "iroha.bond.repay.v1".into(),
        interface: unlisted,
        implementation: bundle,
    });
    assert_eq!(
        selection.check_closure(&catalog),
        Err(SelectionError::BadBinding("iroha.bond.repay.v1".into())),
        "a bundle answers only the interfaces it lists"
    );
}

#[test]
fn installed_storage_namespaces_are_unique() {
    let (mut catalog, _, mut selection) = bond_root();
    let mut twin = bonds_table();
    if let DefinitionDescriptorV1::Storage(storage) = &mut twin {
        storage.invariants = [0x44; 32];
    }
    let twin = publish(&mut catalog, twin);
    selection.installed_storage = sorted([selection.installed_storage, vec![twin]].concat());
    assert!(matches!(
        selection.check_closure(&catalog),
        Err(SelectionError::DuplicateNamespace(_))
    ));
}

#[test]
fn definitions_and_selections_round_trip_through_json() {
    let (catalog, _, selection) = bond_root();
    for descriptor in catalog.values() {
        let json = norito::json::to_json(descriptor).expect("descriptor JSON");
        let back: DefinitionDescriptorV1 = norito::json::from_str(&json).expect("descriptor");
        assert_eq!(&back, descriptor);
    }
    let json = norito::json::to_json(&selection).expect("selection JSON");
    let back: RootSelectionV1 = norito::json::from_str(&json).expect("selection");
    assert_eq!(back, selection);
    assert!(
        json.contains(&selection.kernel.to_string()),
        "identities are hex strings: {json}"
    );
}
