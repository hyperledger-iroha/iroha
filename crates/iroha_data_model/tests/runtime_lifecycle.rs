//! Live-upgrade lifecycle: change sets, attestations, containment and expectations.

use std::collections::BTreeMap;

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey};
use iroha_data_model::{
    NetworkId,
    block::BlockHeader,
    runtime::{
        definition::{
            BindingV1, CompletionClosureV1, DefinitionDescriptorV1, DefinitionId, RootSelectionV1,
            SelectionId,
        },
        lifecycle::{
            ChangeSetEndorsementV1, ChangeSetError, ChangeSetId, ChangeSetV1, ContainmentError,
            ContainmentKindV1, ContainmentRecordV1, ContainmentStatusV1, ContainmentV1,
            ENDORSEMENT_LIFETIME_MS, EndorsementV1, ExpectationV1, ProvenanceShortfall,
            RestrictionKindV1, RootScopeV1, RuntimeRejectionV1, SelectionTransitionV1,
            SupportAttestationV1, check_change_set_provenance, committee_digest,
            committee_supports,
        },
    },
};

use crate::runtime_definition::{bond_root, implementation, interface, publish, sorted};

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([0x11; 32]),
    ))
}

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}

/// Build the change set that turns `predecessor` into `successor` with `transitions`.
fn change_set(
    predecessor: &RootSelectionV1,
    successor: &RootSelectionV1,
    publish: Vec<DefinitionDescriptorV1>,
    transitions: Vec<SelectionTransitionV1>,
) -> ChangeSetV1 {
    ChangeSetV1 {
        network: network(),
        root: RootScopeV1::Universal,
        predecessor: predecessor.id().unwrap(),
        successor: successor.id().unwrap(),
        effective_height: 120,
        publish,
        transitions,
        qualification: [0x5a; 32],
    }
}

/// Add `iroha.bond.call.v1` to the bond root: the selection, the change set and the new ids.
fn add_call() -> (
    BTreeMap<DefinitionId, DefinitionDescriptorV1>,
    RootSelectionV1,
    RootSelectionV1,
    ChangeSetV1,
) {
    let (catalog, ids, predecessor) = bond_root();
    let call_interface = interface("iroha.bond.call.v1", ids.event);
    let call = call_interface.id().unwrap();
    let call_implementation = implementation(&ids, call, "bond.call");
    let call_impl = call_implementation.id().unwrap();

    let mut successor = predecessor.clone();
    successor.active = sorted([successor.active, vec![call, call_impl]].concat());
    successor.bindings.push(BindingV1 {
        wire_id: "iroha.bond.call.v1".into(),
        interface: call,
        implementation: call_impl,
    });
    successor
        .bindings
        .sort_by(|left, right| left.wire_id.cmp(&right.wire_id));

    let mut publish = vec![(call, call_interface), (call_impl, call_implementation)];
    publish.sort_by_key(|(id, _)| *id);
    let change = change_set(
        &predecessor,
        &successor,
        publish
            .into_iter()
            .map(|(_, descriptor)| descriptor)
            .collect(),
        vec![
            SelectionTransitionV1::Activate(call),
            SelectionTransitionV1::Activate(call_impl),
            SelectionTransitionV1::Bind(BindingV1 {
                wire_id: "iroha.bond.call.v1".into(),
                interface: call,
                implementation: call_impl,
            }),
        ],
    );
    (catalog, predecessor, successor, change)
}

#[test]
fn a_change_set_adds_one_instruction() {
    let (catalog, predecessor, successor, change) = add_call();
    let applied = change.apply(&predecessor, &catalog).unwrap();
    assert_eq!(applied.selection, successor);
    assert_eq!(applied.published.len(), 2);
    assert!(
        applied.published.keys().all(|id| !catalog.contains_key(id)),
        "only the new definitions are published"
    );
}

#[test]
fn the_change_set_must_start_and_end_where_it_says() {
    let (catalog, predecessor, _, change) = add_call();

    let mut stale = change.clone();
    stale.predecessor = SelectionId([0; 32]);
    assert_eq!(
        stale.apply(&predecessor, &catalog),
        Err(ChangeSetError::PredecessorMismatch)
    );

    let mut wrong = change.clone();
    wrong.successor = predecessor.id().unwrap();
    assert_eq!(
        wrong.apply(&predecessor, &catalog),
        Err(ChangeSetError::SuccessorMismatch)
    );

    let mut republished = change;
    republished.publish.push(republished.publish[0].clone());
    assert_eq!(
        republished.apply(&predecessor, &catalog),
        Err(ChangeSetError::NonCanonicalPublication)
    );
}

#[test]
fn disabling_needed_code_waits_for_its_obligations() {
    let (catalog, _, selection) = bond_root();
    let claim_impl = selection.bindings[0].implementation;
    let obligation = [0x0b; 32];

    let mut holding = selection.clone();
    holding.completion_closures = vec![CompletionClosureV1 {
        obligation,
        definitions: vec![claim_impl],
    }];
    let mut disabled = holding.clone();
    disabled.restrictions = vec![iroha_data_model::runtime::definition::RestrictionEntryV1 {
        definition: claim_impl,
        restriction: RestrictionKindV1::Disabled.at(120),
    }];
    let stranding = change_set(
        &holding,
        &disabled,
        Vec::new(),
        vec![SelectionTransitionV1::Restrict {
            definition: claim_impl,
            restriction: RestrictionKindV1::Disabled,
        }],
    );
    assert_eq!(
        stranding.apply(&holding, &catalog),
        Err(ChangeSetError::ObligationStranded(claim_impl))
    );

    let mut drained = disabled.clone();
    drained.completion_closures.clear();
    let release_then_disable = change_set(
        &holding,
        &drained,
        Vec::new(),
        vec![
            SelectionTransitionV1::ReleaseCompletion(obligation),
            SelectionTransitionV1::Restrict {
                definition: claim_impl,
                restriction: RestrictionKindV1::Disabled,
            },
        ],
    );
    assert_eq!(
        release_then_disable
            .apply(&holding, &catalog)
            .unwrap()
            .selection,
        drained
    );

    let admission_closed = change_set(
        &holding,
        &holding,
        Vec::new(),
        vec![SelectionTransitionV1::Deactivate(claim_impl)],
    );
    assert!(matches!(
        admission_closed.apply(&holding, &catalog),
        Err(ChangeSetError::Transition { .. })
    ));
}

#[test]
fn rebinding_breaks_a_signed_expectation() {
    let (mut catalog, ids, selection) = bond_root();
    let claim = selection
        .active
        .iter()
        .copied()
        .find(|id| matches!(catalog[id], DefinitionDescriptorV1::Interface(_)))
        .unwrap();
    let old_impl = selection.bindings[0].implementation;
    let expectation = ExpectationV1 {
        bindings: vec![BindingV1 {
            wire_id: "iroha.bond.claim.v1".into(),
            interface: claim,
            implementation: old_impl,
        }],
        services_profile: selection.services_profile,
        kernel: selection.kernel,
        commitment_profile: selection.commitment_profile,
        definitions: sorted(vec![claim, old_impl, selection.services_profile]),
        exact_bindings: false,
    };
    assert_eq!(
        norito::decode_from_bytes::<ExpectationV1>(&norito::to_bytes(&expectation).unwrap())
            .unwrap(),
        expectation,
    );
    assert_eq!(
        norito::json::from_str::<ExpectationV1>(&norito::json::to_json(&expectation).unwrap())
            .unwrap(),
        expectation,
    );
    assert_eq!(expectation.check(&selection), Ok(()));

    let fixed = implementation(&ids, claim, "bond.claim.fixed");
    let fixed_id = publish(&mut catalog, fixed);
    let mut rebound = selection.clone();
    rebound.active = sorted([rebound.active, vec![fixed_id]].concat());
    rebound.bindings[0].implementation = fixed_id;
    let rebind = change_set(
        &selection,
        &rebound,
        Vec::new(),
        vec![
            SelectionTransitionV1::Activate(fixed_id),
            SelectionTransitionV1::Bind(BindingV1 {
                wire_id: "iroha.bond.claim.v1".into(),
                interface: claim,
                implementation: fixed_id,
            }),
        ],
    );
    assert_eq!(
        rebind.apply(&selection, &catalog).unwrap().selection,
        rebound
    );
    assert_eq!(
        expectation.check(&rebound),
        Err(RuntimeRejectionV1::ExpectationMismatch {
            wire_id: Some("iroha.bond.claim.v1".into())
        })
    );
}

fn attestation(
    change: &ChangeSetV1,
    committee: [u8; 32],
    validator: PublicKey,
) -> SupportAttestationV1 {
    SupportAttestationV1 {
        network: network(),
        root: RootScopeV1::Universal,
        change_set: change.id().unwrap(),
        predecessor: change.predecessor,
        successor: change.successor,
        committee,
        tested_checkpoint: 100,
        valid_from: 101,
        valid_until: 200,
        release: [0x2e; 32],
        validator,
    }
}

#[test]
fn activation_needs_every_validator() {
    let (_, _, _, change) = add_call();
    let id = change.id().unwrap();
    let validators: Vec<KeyPair> = (1..=4).map(key).collect();
    let committee: Vec<PublicKey> = validators
        .iter()
        .map(|pair| pair.public_key().clone())
        .collect();
    let digest = committee_digest(&committee).unwrap();
    let signed: Vec<_> = validators
        .iter()
        .map(|pair| {
            attestation(&change, digest, pair.public_key().clone())
                .sign(pair.private_key())
                .unwrap()
        })
        .collect();

    assert!(committee_supports(&committee, &signed, id, 150));
    assert!(
        !committee_supports(&committee, &signed[..3], id, 150),
        "three of four is a quorum, not readiness"
    );
    assert!(!committee_supports(&committee, &signed, id, 200), "expired");
    assert!(
        !committee_supports(&committee[1..], &signed[1..], id, 150),
        "another committee"
    );

    let mut forged = signed.clone();
    forged[0].attestation.successor = SelectionId([0x99; 32]);
    assert!(forged[0].verify().is_err());
    assert!(!committee_supports(&committee, &forged, id, 150));
}

#[test]
fn change_set_identities_are_hex_strings_in_json() {
    let id = ChangeSetId([0xab; 32]);
    let json = norito::json::to_json(&id).unwrap();
    assert_eq!(json, format!("\"{}\"", "ab".repeat(32)));
    assert_eq!(norito::json::from_str::<ChangeSetId>(&json).unwrap(), id);
}

#[test]
fn containment_renewal_opens_halfway_through_the_lease() {
    let record = ContainmentRecordV1 {
        containment: ContainmentV1 {
            network: network(),
            root: RootScopeV1::Universal,
            incident: [0x2c; 32],
            targets: vec![DefinitionId([3; 32])],
            kind: ContainmentKindV1::AdmissionClosed,
            lease_until_ms: 1_000 + 172_800_000,
        },
        authorized_height: 7,
        authorized_ms: 1_000,
        contained: Vec::new(),
        status: ContainmentStatusV1::Active,
        review: None,
    };
    assert_eq!(record.renewal_opens_ms(), 1_000 + 86_400_000);
}

#[test]
fn the_committee_digest_names_members_not_their_order() {
    let committee: Vec<PublicKey> = (1..=4).map(|seed| key(seed).public_key().clone()).collect();
    let mut rotated = committee.clone();
    rotated.rotate_left(1);
    assert_eq!(
        committee_digest(&committee).unwrap(),
        committee_digest(&rotated).unwrap(),
        "a leader rotation keeps the attestations"
    );
    let mut replaced = committee.clone();
    replaced[3] = key(9).public_key().clone();
    assert_ne!(
        committee_digest(&committee).unwrap(),
        committee_digest(&replaced).unwrap(),
        "a membership change needs new attestations"
    );
}

#[test]
fn containment_needs_fresh_endorsements_from_the_panel() {
    let panel_keys: Vec<KeyPair> = (20..29).map(key).collect();
    let panel: Vec<PublicKey> = panel_keys
        .iter()
        .map(|pair| pair.public_key().clone())
        .collect();
    let containment = ContainmentV1 {
        network: network(),
        root: RootScopeV1::Universal,
        incident: [0x1c; 32],
        targets: vec![DefinitionId([3; 32]), DefinitionId([4; 32])],
        kind: ContainmentKindV1::AdmissionClosed,
        lease_until_ms: 1_000_000_000,
    };
    let id = containment.id().unwrap();
    let now = 10_000_000;
    let endorse = |pair: &KeyPair, at: u64| {
        EndorsementV1::sign(id, at, pair.public_key().clone(), pair.private_key()).unwrap()
    };

    let six: Vec<_> = panel_keys[..6]
        .iter()
        .map(|pair| endorse(pair, now - 60_000))
        .collect();
    assert_eq!(containment.check_endorsements(&panel, 6, now, &six), Ok(()));

    let mut duplicated = six[..5].to_vec();
    duplicated.push(six[0].clone());
    assert_eq!(
        containment.check_endorsements(&panel, 6, now, &duplicated),
        Err(ContainmentError::Threshold { have: 5, need: 6 })
    );

    let mut stale = six[..5].to_vec();
    stale.push(endorse(&panel_keys[5], now - ENDORSEMENT_LIFETIME_MS - 1));
    assert_eq!(
        containment.check_endorsements(&panel, 6, now, &stale),
        Err(ContainmentError::Threshold { have: 5, need: 6 })
    );

    let outsider = key(99);
    let mut foreign = six[..5].to_vec();
    foreign.push(endorse(&outsider, now - 60_000));
    assert_eq!(
        containment.check_endorsements(&panel, 6, now, &foreign),
        Err(ContainmentError::Threshold { have: 5, need: 6 })
    );

    let unordered = ContainmentV1 {
        targets: vec![DefinitionId([4; 32]), DefinitionId([3; 32])],
        ..containment
    };
    assert_eq!(
        unordered.check_endorsements(&panel, 6, now, &six),
        Err(ContainmentError::Targets)
    );
}

#[test]
fn change_set_provenance_needs_distinct_trusted_signers() {
    let change_set = ChangeSetId([0x42; 32]);
    let mut signers: Vec<PublicKey> = (1..=3).map(|seed| key(seed).public_key().clone()).collect();
    signers.sort();
    let endorse = |seed: u8, change_set: ChangeSetId| {
        let pair = key(seed);
        ChangeSetEndorsementV1::sign(change_set, pair.public_key().clone(), pair.private_key())
            .expect("endorse")
    };
    let one = endorse(1, change_set);
    one.verify().expect("the signer's signature verifies");
    assert_eq!(
        check_change_set_provenance(change_set, &signers, 2, &[one.clone(), one.clone()]),
        Err(ProvenanceShortfall { have: 1, need: 2 }),
        "a repeated endorsement counts once"
    );
    assert_eq!(
        check_change_set_provenance(
            change_set,
            &signers,
            2,
            &[
                one.clone(),
                endorse(9, change_set),
                endorse(2, ChangeSetId([0x43; 32]))
            ],
        ),
        Err(ProvenanceShortfall { have: 1, need: 2 }),
        "untrusted keys and endorsements of another change set do not count"
    );
    let mut forged = endorse(2, change_set);
    forged.signer = key(3).public_key().clone();
    assert!(forged.verify().is_err());
    assert_eq!(
        check_change_set_provenance(change_set, &signers, 2, &[one.clone(), forged]),
        Err(ProvenanceShortfall { have: 1, need: 2 }),
        "a signature by another key does not count"
    );
    assert_eq!(
        check_change_set_provenance(change_set, &signers, 2, &[one, endorse(3, change_set)]),
        Ok(())
    );
}

#[test]
fn rejection_codes_are_stable() {
    assert_eq!(
        RuntimeRejectionV1::DefinitionInactive {
            wire_id: "iroha.bond.call.v1".into()
        }
        .code(),
        "definition_inactive"
    );
    assert_eq!(
        RuntimeRejectionV1::DefinitionRestricted {
            definition: DefinitionId([1; 32]),
            restriction: RestrictionKindV1::Disabled.at(9),
        }
        .code(),
        "definition_restricted"
    );
    assert_eq!(
        RuntimeRejectionV1::ExpectationMismatch { wire_id: None }.code(),
        "expectation_mismatch"
    );
    assert_eq!(
        RuntimeRejectionV1::MalformedPayload {
            wire_id: "iroha.bond.call.v1".into()
        }
        .code(),
        "malformed_payload"
    );
}

#[test]
fn malformed_payload_rejection_round_trips_and_names_its_wire_id() {
    let rejection = RuntimeRejectionV1::MalformedPayload {
        wire_id: "iroha.bond.call.v1".into(),
    };
    let bytes = norito::to_bytes(&rejection).expect("encode rejection");
    assert_eq!(
        norito::decode_from_bytes::<RuntimeRejectionV1>(&bytes).expect("decode rejection"),
        rejection
    );
    let json = norito::json::to_json(&rejection).expect("rejection JSON");
    assert_eq!(
        norito::json::from_str::<RuntimeRejectionV1>(&json).expect("rejection from JSON"),
        rejection
    );
    assert!(rejection.to_string().contains("iroha.bond.call.v1"));
}

/// The JSON of a change set, whose shapes the published OpenAPI schema closes.
///
/// `scripts/tests/check_openapi_fail_closed_test.py` validates this same fixture against that
/// schema, so a new variant reaches the schema through the coverage check here.
mod change_set_json_fixture {
    use std::collections::BTreeSet;

    use iroha_data_model::runtime::{
        definition::{
            BindingV1, CarrierFieldV1, CarrierNodeV1, CarrierSchemaV1, CarrierVariantV1,
            CommitmentProfileDescriptorV1, CompletionClosureV1, DefinitionDescriptorV1,
            DefinitionId, EntrypointV1, EventDescriptorV1, HostProfileDescriptorV1,
            ImplementationCodeV1, ImplementationDescriptorV1, InterfaceDescriptorV1,
            KernelDescriptorV1, MigrationDescriptorV1, ParameterDescriptorV1,
            PermissionDescriptorV1, ProjectionV1, ResourceLimitsV1, SchemaV1, SelectionId,
            ServiceEntryV1, ServicesProfileDescriptorV1, StorageDescriptorV1, StorageShapeV1,
        },
        lifecycle::{ChangeSetV1, RestrictionKindV1, RootScopeV1, SelectionTransitionV1},
    };
    use iroha_schema::IntoSchema;
    use norito::json::Value;

    const FIXTURE: &str = include_str!("../../../fixtures/governance/runtime_change_set_v1.json");
    const LIMITS: ResourceLimitsV1 = ResourceLimitsV1 {
        max_gas: u64::MAX,
        max_effects: 64,
        max_call_depth: 8,
    };
    const WIRE_ID: &str = "iroha.fixture.call.v1";

    fn id(seed: u8) -> DefinitionId {
        DefinitionId([seed; 32])
    }

    fn codec(name: &str) -> SchemaV1 {
        SchemaV1::Codec(format!("norito:{name}@1.0.0"))
    }

    fn every_carrier_node() -> SchemaV1 {
        SchemaV1::Carrier(CarrierSchemaV1 {
            nodes: vec![
                CarrierNodeV1::Unit,
                CarrierNodeV1::Bool,
                CarrierNodeV1::U8,
                CarrierNodeV1::U16,
                CarrierNodeV1::U32,
                CarrierNodeV1::U64,
                CarrierNodeV1::U128,
                CarrierNodeV1::I64,
                CarrierNodeV1::String { max_bytes: 64 },
                CarrierNodeV1::Bytes { max_bytes: 64 },
                CarrierNodeV1::AccountId,
                CarrierNodeV1::AssetDefinitionId,
                CarrierNodeV1::Hash,
                CarrierNodeV1::Option { inner: 0 },
                CarrierNodeV1::Vec {
                    element: 1,
                    max_len: 8,
                },
                CarrierNodeV1::Map {
                    key: 2,
                    value: 3,
                    max_len: 8,
                },
                CarrierNodeV1::Struct {
                    fields: vec![CarrierFieldV1 {
                        name: "first".into(),
                        node: 4,
                    }],
                },
                CarrierNodeV1::Enum {
                    variants: vec![CarrierVariantV1 {
                        discriminant: 0,
                        name: "only".into(),
                        node: 16,
                    }],
                },
            ],
        })
    }

    fn native_code(module: &str) -> ImplementationCodeV1 {
        ImplementationCodeV1::Native {
            module: module.into(),
            reference_closure: [4; 32],
        }
    }

    fn implementation(
        entrypoint: EntrypointV1,
        code: ImplementationCodeV1,
        host_profile: Option<DefinitionId>,
    ) -> DefinitionDescriptorV1 {
        DefinitionDescriptorV1::Implementation(ImplementationDescriptorV1 {
            entrypoint,
            code,
            reads: vec![id(5)],
            writes: vec![id(5)],
            scans: vec![id(5)],
            events: vec![id(1)],
            permissions: vec![id(6)],
            parameters: vec![id(7)],
            invokes: vec![id(3)],
            host_profile,
            services_profile: id(8),
            limits: LIMITS,
        })
    }

    /// One value of every JSON shape a published definition or a transition can take. It is a
    /// codec fixture that nothing applies, so its identities are arbitrary.
    fn every_json_shape() -> ChangeSetV1 {
        ChangeSetV1 {
            network: super::network(),
            root: RootScopeV1::Universal,
            predecessor: SelectionId([0x21; 32]),
            successor: SelectionId([0x22; 32]),
            effective_height: 120,
            publish: vec![
                DefinitionDescriptorV1::Interface(InterfaceDescriptorV1 {
                    wire_id: WIRE_ID.into(),
                    payload: codec("iroha.fixture.Call"),
                    rejection_codes: vec!["fixture_rejected".into()],
                    events: vec![id(1)],
                    metering_class: "iroha.fixture".into(),
                    contract_statement: [2; 32],
                }),
                implementation(
                    EntrypointV1::Instructions(vec![id(3)]),
                    native_code("iroha.fixture.instructions"),
                    None,
                ),
                implementation(
                    EntrypointV1::Hook("iroha.fixture.hook".into()),
                    ImplementationCodeV1::Ivm {
                        code_hash: [9; 32],
                        entrypoint: "main".into(),
                    },
                    Some(id(10)),
                ),
                implementation(
                    EntrypointV1::Query("iroha.fixture.query".into()),
                    native_code("iroha.fixture.query"),
                    None,
                ),
                DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
                    namespace: "world.fixture_table".into(),
                    shape: StorageShapeV1::Table {
                        key: codec("iroha.fixture.Key"),
                    },
                    value: codec("iroha.fixture.Value"),
                    initial: None,
                    invariants: [11; 32],
                    projection: ProjectionV1::BareNorito,
                }),
                DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
                    namespace: "world.fixture_cell".into(),
                    shape: StorageShapeV1::Cell,
                    value: codec("iroha.fixture.Value"),
                    initial: Some(vec![1, 2, 3]),
                    invariants: [11; 32],
                    projection: ProjectionV1::Semantic("iroha.fixture.projection@1.0.0".into()),
                }),
                DefinitionDescriptorV1::Event(EventDescriptorV1 {
                    name: "iroha.fixture.event".into(),
                    schema: every_carrier_node(),
                }),
                DefinitionDescriptorV1::Permission(PermissionDescriptorV1 {
                    name: "iroha.fixture.permission".into(),
                    payload: codec("iroha.fixture.Permission"),
                }),
                DefinitionDescriptorV1::Parameter(ParameterDescriptorV1 {
                    name: "iroha.fixture.parameter".into(),
                    schema: codec("iroha.fixture.Parameter"),
                    initial: vec![4, 5],
                }),
                DefinitionDescriptorV1::HostProfile(HostProfileDescriptorV1 {
                    abi_version: 1,
                    abi_hash: [12; 32],
                    metering: [13; 32],
                }),
                DefinitionDescriptorV1::ServicesProfile(ServicesProfileDescriptorV1 {
                    services: vec![ServiceEntryV1 {
                        name: "iroha.fixture.service".into(),
                        reference_closure: [14; 32],
                    }],
                }),
                DefinitionDescriptorV1::CommitmentProfile(CommitmentProfileDescriptorV1 {
                    scheme: "iroha.fixture.scheme".into(),
                    parameters: [15; 32],
                }),
                DefinitionDescriptorV1::Kernel(KernelDescriptorV1 {
                    name: "iroha.fixture.kernel".into(),
                    reference_closure: [16; 32],
                }),
                DefinitionDescriptorV1::Migration(MigrationDescriptorV1 {
                    code: native_code("iroha.fixture.migration"),
                    from: vec![id(17)],
                    to: vec![id(18)],
                    cursor: codec("iroha.fixture.Cursor"),
                    host_profile: None,
                    services_profile: id(8),
                    limits: LIMITS,
                }),
            ],
            transitions: vec![
                SelectionTransitionV1::InstallStorage(id(5)),
                SelectionTransitionV1::RetireStorage(id(19)),
                SelectionTransitionV1::Activate(id(3)),
                SelectionTransitionV1::Deactivate(id(20)),
                SelectionTransitionV1::Bind(BindingV1 {
                    wire_id: WIRE_ID.into(),
                    interface: id(3),
                    implementation: id(21),
                }),
                SelectionTransitionV1::Unbind("iroha.fixture.retired.v1".into()),
                SelectionTransitionV1::Restrict {
                    definition: id(20),
                    restriction: RestrictionKindV1::Deprecated,
                },
                SelectionTransitionV1::Restrict {
                    definition: id(20),
                    restriction: RestrictionKindV1::AdmissionClosed,
                },
                SelectionTransitionV1::Restrict {
                    definition: id(20),
                    restriction: RestrictionKindV1::CompletionOnly,
                },
                SelectionTransitionV1::Restrict {
                    definition: id(20),
                    restriction: RestrictionKindV1::Disabled,
                },
                SelectionTransitionV1::Lift(id(20)),
                SelectionTransitionV1::SetKernel(id(22)),
                SelectionTransitionV1::SetCommitmentProfile(id(23)),
                SelectionTransitionV1::SetServicesProfile(id(8)),
                SelectionTransitionV1::RetainCompletion(CompletionClosureV1 {
                    obligation: [24; 32],
                    definitions: vec![id(3)],
                }),
                SelectionTransitionV1::ReleaseCompletion([24; 32]),
            ],
            qualification: [0x5a; 32],
        }
    }

    fn variant_tags<T: IntoSchema + 'static>() -> Vec<String> {
        let schema = T::schema();
        let Some(iroha_schema::Metadata::Enum(metadata)) = schema.get::<T>() else {
            panic!("{} must keep its closed enum schema", T::type_name());
        };
        metadata
            .variants
            .iter()
            .map(|variant| variant.tag.clone())
            .collect()
    }

    fn collect_kinds(value: &Value, kinds: &mut BTreeSet<String>) {
        match value {
            Value::Object(map) => {
                if let Some(Value::String(kind)) = map.get("kind") {
                    kinds.insert(kind.clone());
                }
                for child in map.values() {
                    collect_kinds(child, kinds);
                }
            }
            Value::Array(items) => items.iter().for_each(|child| collect_kinds(child, kinds)),
            _ => {}
        }
    }

    #[test]
    fn the_fixture_is_the_encoder_output_of_every_definition_and_transition_shape() {
        let change_set = every_json_shape();
        for definition in &change_set.publish {
            definition
                .validate()
                .expect("the fixture's definitions follow their format rules");
        }
        let encoded = norito::json::to_json(&change_set).expect("encode the fixture change set");
        let encoded_value: Value = norito::json::from_str(&encoded).expect("encoded JSON parses");
        let fixture_value: Value = norito::json::from_str(FIXTURE).expect("fixture JSON parses");
        assert_eq!(
            encoded_value, fixture_value,
            "fixtures/governance/runtime_change_set_v1.json is not the encoder's output: {encoded}"
        );
        let decoded: ChangeSetV1 = norito::json::from_str(FIXTURE).expect("fixture decodes");
        assert_eq!(decoded, change_set);

        let mut kinds = BTreeSet::new();
        collect_kinds(&fixture_value, &mut kinds);
        for tag in [
            variant_tags::<DefinitionDescriptorV1>(),
            variant_tags::<SelectionTransitionV1>(),
            variant_tags::<SchemaV1>(),
            variant_tags::<CarrierNodeV1>(),
            variant_tags::<ImplementationCodeV1>(),
            variant_tags::<EntrypointV1>(),
            variant_tags::<StorageShapeV1>(),
            variant_tags::<ProjectionV1>(),
            variant_tags::<RestrictionKindV1>(),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                kinds.contains(&tag),
                "the fixture and the OpenAPI schema need the `{tag}` variant"
            );
        }
    }
}
