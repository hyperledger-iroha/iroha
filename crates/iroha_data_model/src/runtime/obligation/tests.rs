//! Completion captures over a small definition graph: a bundle that answers three interfaces
//! and one addition that rebinds one of them.

use std::collections::BTreeMap;

use iroha_crypto::{Algorithm, KeyPair};

use super::*;
use crate::runtime::definition::{
    CommitmentProfileDescriptorV1, EventDescriptorV1, HostProfileDescriptorV1,
    ImplementationCodeV1, ImplementationDescriptorV1, InterfaceDescriptorV1, KernelDescriptorV1,
    ProjectionV1, ResourceLimitsV1, RestrictionEntryV1, SchemaV1, ServiceEntryV1,
    ServicesProfileDescriptorV1, StorageDescriptorV1, StorageShapeV1,
};

type Catalog = BTreeMap<DefinitionId, DefinitionDescriptorV1>;

const TRANSFER: &str = "iroha.test.transfer";
const MINT: &str = "iroha.test.mint";
const LOG: &str = "iroha.test.log";

fn codec(name: &str) -> SchemaV1 {
    SchemaV1::Codec(format!("norito:{name}@0.0.0"))
}

fn publish(catalog: &mut Catalog, descriptor: DefinitionDescriptorV1) -> DefinitionId {
    let id = descriptor.id().expect("valid descriptor");
    catalog.insert(id, descriptor);
    id
}

fn sorted(mut ids: Vec<DefinitionId>) -> Vec<DefinitionId> {
    ids.sort_unstable();
    ids
}

fn interface(catalog: &mut Catalog, wire_id: &str, event: DefinitionId) -> DefinitionId {
    publish(
        catalog,
        DefinitionDescriptorV1::Interface(InterfaceDescriptorV1 {
            wire_id: wire_id.to_owned(),
            payload: codec("test::Payload"),
            rejection_codes: Vec::new(),
            events: vec![event],
            metering_class: "native.test".to_owned(),
            contract_statement: [7; 32],
        }),
    )
}

struct Root {
    catalog: Catalog,
    selection: RootSelectionV1,
    table: DefinitionId,
    host: DefinitionId,
    bundle: DefinitionId,
    fix: DefinitionId,
    interfaces: [DefinitionId; 3],
}

fn implementation(
    catalog: &mut Catalog,
    module: &str,
    answers: Vec<DefinitionId>,
    invokes: Vec<DefinitionId>,
    table: DefinitionId,
    event: DefinitionId,
    services: DefinitionId,
) -> DefinitionId {
    publish(
        catalog,
        DefinitionDescriptorV1::Implementation(ImplementationDescriptorV1 {
            entrypoint: EntrypointV1::Instructions(sorted(answers)),
            code: ImplementationCodeV1::Native {
                module: module.to_owned(),
                reference_closure: [8; 32],
            },
            reads: vec![table],
            writes: vec![table],
            scans: Vec::new(),
            events: vec![event],
            permissions: Vec::new(),
            parameters: Vec::new(),
            invokes: sorted(invokes),
            host_profile: None,
            services_profile: services,
            limits: ResourceLimitsV1 {
                max_gas: 1,
                max_effects: 1,
                max_call_depth: 1,
            },
        }),
    )
}

/// A root whose bundle answers `TRANSFER`, `MINT` and `LOG` and invokes all three, with a
/// published fix that answers `LOG` alone. `rebound` binds `LOG` to the fix.
fn root(rebound: bool) -> Root {
    let mut catalog = Catalog::new();
    let kernel = publish(
        &mut catalog,
        DefinitionDescriptorV1::Kernel(KernelDescriptorV1 {
            name: "iroha.test.kernel".to_owned(),
            reference_closure: [1; 32],
        }),
    );
    let commitment_profile = publish(
        &mut catalog,
        DefinitionDescriptorV1::CommitmentProfile(CommitmentProfileDescriptorV1 {
            scheme: "iroha.test.commitment".to_owned(),
            parameters: [2; 32],
        }),
    );
    let services_profile = publish(
        &mut catalog,
        DefinitionDescriptorV1::ServicesProfile(ServicesProfileDescriptorV1 {
            services: vec![ServiceEntryV1 {
                name: "authorization".to_owned(),
                reference_closure: [3; 32],
            }],
        }),
    );
    let table = publish(
        &mut catalog,
        DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
            namespace: "world.test".to_owned(),
            shape: StorageShapeV1::Table {
                key: codec("test::Key"),
            },
            value: codec("test::Value"),
            initial: None,
            invariants: [4; 32],
            projection: ProjectionV1::BareNorito,
        }),
    );
    let event = publish(
        &mut catalog,
        DefinitionDescriptorV1::Event(EventDescriptorV1 {
            name: "iroha.test.event".to_owned(),
            schema: codec("test::Event"),
        }),
    );
    let host = publish(
        &mut catalog,
        DefinitionDescriptorV1::HostProfile(HostProfileDescriptorV1 {
            abi_version: 1,
            abi_hash: [5; 32],
            metering: [6; 32],
        }),
    );
    let interfaces = [TRANSFER, MINT, LOG].map(|wire_id| interface(&mut catalog, wire_id, event));
    let bundle = implementation(
        &mut catalog,
        "iroha.test.bundle",
        interfaces.to_vec(),
        interfaces.to_vec(),
        table,
        event,
        services_profile,
    );
    let fix = implementation(
        &mut catalog,
        "iroha.test.fix",
        vec![interfaces[2]],
        interfaces.to_vec(),
        table,
        event,
        services_profile,
    );
    let mut active = vec![event, host, bundle];
    active.extend(interfaces);
    if rebound {
        active.push(fix);
    }
    let mut bindings: Vec<BindingV1> = [TRANSFER, MINT, LOG]
        .iter()
        .zip(interfaces)
        .map(|(wire_id, interface)| BindingV1 {
            wire_id: (*wire_id).to_owned(),
            interface,
            implementation: if rebound && *wire_id == LOG {
                fix
            } else {
                bundle
            },
        })
        .collect();
    bindings.sort_by(|left, right| left.wire_id.cmp(&right.wire_id));
    let selection = RootSelectionV1 {
        kernel,
        commitment_profile,
        services_profile,
        installed_storage: vec![table],
        active: sorted(active),
        bindings,
        restrictions: Vec::new(),
        completion_closures: Vec::new(),
    };
    selection.check_closure(&catalog).expect("closed selection");
    Root {
        catalog,
        selection,
        table,
        host,
        bundle,
        fix,
        interfaces,
    }
}

fn effects(wire_ids: &[&str]) -> Vec<String> {
    wire_ids
        .iter()
        .map(|wire_id| (*wire_id).to_owned())
        .collect()
}

#[test]
fn a_capture_resolves_every_reached_interface_as_its_selection_bound_it() {
    let root = root(true);
    let capture =
        CompletionCaptureV1::capture(&root.selection, &root.catalog, &effects(&[TRANSFER]), 10)
            .expect("capture");
    capture.validate().expect("canonical capture");
    assert_eq!(capture.terminal_effects().collect::<Vec<_>>(), [TRANSFER]);
    assert_eq!(capture.default_implementation, Some(root.bundle));
    assert_eq!(
        capture
            .nested
            .iter()
            .map(|bound| (bound.wire_id.as_str(), bound.implementation))
            .collect::<Vec<_>>(),
        [(LOG, root.fix)],
        "only the interface bound away from the default is listed"
    );
    assert_eq!(capture.host_profiles, [root.host]);
    let resolve = |wire_id: &str, index: usize| {
        capture.implementation_for(wire_id, &root.interfaces[index], &root.catalog)
    };
    assert_eq!(resolve(TRANSFER, 0), Some(root.bundle));
    assert_eq!(resolve(MINT, 1), Some(root.bundle));
    assert_eq!(resolve(LOG, 2), Some(root.fix));
    assert_eq!(
        capture.implementation_for(LOG, &root.interfaces[0], &root.catalog),
        None,
        "a wire ID answered by another interface is outside the closure"
    );
    assert_eq!(
        capture.implementation_for("iroha.test.other", &DefinitionId([9; 32]), &root.catalog),
        None
    );

    let closure = capture.closure(&root.catalog).expect("closure");
    for needed in [
        root.selection.kernel,
        root.selection.commitment_profile,
        root.selection.services_profile,
        root.table,
        root.bundle,
        root.fix,
        root.host,
    ]
    .into_iter()
    .chain(root.interfaces)
    {
        assert!(closure.contains(&needed), "{needed}");
    }
}

#[test]
fn a_capture_survives_the_rebinding_it_predates() {
    let before = root(false);
    let capture = CompletionCaptureV1::capture(
        &before.selection,
        &before.catalog,
        &effects(&[LOG, TRANSFER]),
        10,
    )
    .expect("capture");
    assert_eq!(capture.host_profiles, [before.host]);
    assert!(capture.nested.is_empty());
    // The root later rebinds LOG to the fix. The capture still resolves it to the bundle.
    let after = root(true);
    assert_ne!(
        after.selection.bindings, before.selection.bindings,
        "the root rebound LOG"
    );
    assert_eq!(
        capture.implementation_for(LOG, &after.interfaces[2], &after.catalog),
        Some(before.bundle)
    );
    let closure = capture.closure(&after.catalog).expect("closure");
    assert!(closure.contains(&before.bundle) && closure.contains(&before.host));
    assert!(
        !closure.contains(&after.fix),
        "the closure names what was bound at registration, not the later fix"
    );
}

#[test]
fn a_live_obligation_keeps_its_closure_as_it_captured_it() {
    let before = root(false);
    let capture = CompletionCaptureV1::capture(
        &before.selection,
        &before.catalog,
        &effects(&[TRANSFER]),
        10,
    )
    .unwrap();
    let broken = |successor: &RootSelectionV1, catalog: &Catalog| {
        capture
            .broken_by(&before.selection, successor, catalog)
            .expect("published closure")
    };
    assert_eq!(broken(&before.selection, &before.catalog), None);

    // Activating an unrelated implementation changes nothing the closure reaches.
    let after = root(true);
    let mut added = before.selection.clone();
    added.active.push(after.fix);
    added.active.sort_unstable();
    assert_eq!(broken(&added, &after.catalog), None);

    // Rebinding an interface the closure reaches lapses: the bundle invokes LOG.
    let reason = broken(&after.selection, &after.catalog).expect("LOG was rebound");
    assert!(reason.contains(LOG), "{reason}");

    // Disabling or deactivating a captured definition lapses.
    let mut disabled = before.selection.clone();
    disabled.restrictions = vec![RestrictionEntryV1 {
        definition: before.bundle,
        restriction: RestrictionV1::Disabled(20),
    }];
    let reason = broken(&disabled, &before.catalog).expect("the bundle is disabled");
    assert!(reason.contains(&before.bundle.to_string()), "{reason}");
    let mut retired = before.selection.clone();
    retired.installed_storage.clear();
    assert!(broken(&retired, &before.catalog).is_some());
    let mut another_kernel = before.selection.clone();
    another_kernel.kernel = DefinitionId([0xee; 32]);
    assert!(broken(&another_kernel, &before.catalog).is_some());

    // Admission closure and completion-only keep the closure executing.
    let mut restricted = before.selection.clone();
    restricted.restrictions = vec![RestrictionEntryV1 {
        definition: before.bundle,
        restriction: RestrictionV1::CompletionOnly(20),
    }];
    assert_eq!(broken(&restricted, &before.catalog), None);
}

#[test]
fn registering_is_new_work() {
    let root = root(false);
    let capture = |selection: &RootSelectionV1, wire_ids: &[&str]| {
        CompletionCaptureV1::capture(selection, &root.catalog, &effects(wire_ids), 10)
    };
    assert_eq!(
        capture(&root.selection, &["iroha.test.unbound"]),
        Err(ObligationError::Unbound("iroha.test.unbound".to_owned()))
    );
    let restricted = |definition, restriction| {
        let mut selection = root.selection.clone();
        selection.restrictions = vec![RestrictionEntryV1 {
            definition,
            restriction,
        }];
        selection
    };
    for restriction in [
        RestrictionV1::AdmissionClosed(10),
        RestrictionV1::CompletionOnly(3),
        RestrictionV1::Disabled(10),
    ] {
        for definition in [root.bundle, root.interfaces[0]] {
            assert_eq!(
                capture(&restricted(definition, restriction), &[TRANSFER]),
                Err(ObligationError::Restricted(definition)),
                "{restriction:?}"
            );
        }
    }
    assert!(
        capture(
            &restricted(root.interfaces[1], RestrictionV1::AdmissionClosed(10)),
            &[TRANSFER]
        )
        .is_ok(),
        "a restriction elsewhere in the closure does not stop a registration"
    );
    assert!(
        capture(
            &restricted(root.bundle, RestrictionV1::AdmissionClosed(11)),
            &[TRANSFER]
        )
        .is_ok(),
        "a restriction that has not taken effect stops nothing"
    );
    assert!(
        capture(
            &restricted(root.bundle, RestrictionV1::Deprecated(1)),
            &[TRANSFER]
        )
        .is_ok()
    );
}

fn log(message: &str) -> InstructionBox {
    InstructionBox::from(crate::isi::Log::new(crate::Level::INFO, message.to_owned()))
}

#[test]
fn a_registration_commits_to_exact_instructions() {
    let (committed, wire_ids) = terminal_effects(&[log("release"), log("refund")]).unwrap();
    assert_eq!(
        wire_ids,
        [crate::isi::instruction_wire_id(&log("x")).unwrap()]
    );
    assert_eq!(committed.len(), 2);
    assert!(committed[0].envelope < committed[1].envelope);
    assert!(committed.iter().all(|effect| !effect.performed));
    assert_eq!(
        terminal_effect_id(&log("release")),
        terminal_effect_id(&log("release"))
    );
    assert_ne!(
        terminal_effect_id(&log("release")),
        terminal_effect_id(&log("refund")),
        "another payload is another terminal effect"
    );

    let lifecycle = InstructionBox::from(
        crate::isi::runtime_obligation::CompleteRuntimeObligationV1 {
            owner: AccountId::new(
                KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone(),
            ),
        },
    );
    let too_many: Vec<InstructionBox> = (0..=MAX_TERMINAL_EFFECTS)
        .map(|index| log(&format!("effect {index}")))
        .collect();
    for (case, instructions) in [
        ("none", Vec::new()),
        ("repeated", vec![log("release"), log("release")]),
        ("a lifecycle instruction", vec![lifecycle]),
        ("too many", too_many),
    ] {
        assert_eq!(
            terminal_effects(&instructions),
            Err(ObligationError::TerminalEffects),
            "{case}"
        );
    }

    let ok = |kind, expires_ms| check_registration(kind, expires_ms, 1_000);
    assert_eq!(ok(ObligationKindV1::Escrow, 1_001), Ok(()));
    assert_eq!(
        ok(
            ObligationKindV1::PendingConsent,
            1_000 + MAX_OBLIGATION_LIFETIME_MS
        ),
        Ok(())
    );
    for kind in [
        ObligationKindV1::MigrationCursor,
        ObligationKindV1::AmxParticipation,
    ] {
        assert_eq!(ok(kind, 1_001), Err(ObligationError::Kind(kind)));
    }
    for expires_ms in [0, 1_000, 1_001 + MAX_OBLIGATION_LIFETIME_MS] {
        assert_eq!(
            ok(ObligationKindV1::Escrow, expires_ms),
            Err(ObligationError::Expiry)
        );
    }
}

#[test]
fn an_obligation_is_live_until_it_expires_or_performs_every_effect() {
    let root = root(false);
    let owner = AccountId::new(
        KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let (committed, _) = terminal_effects(&[log("release"), log("refund")]).unwrap();
    let mut record = ObligationRecordV1 {
        owner: owner.clone(),
        contract: None,
        kind: ObligationKindV1::Escrow,
        created_at: 10,
        expires_ms: 5_000,
        capture: CompletionCaptureV1::capture(
            &root.selection,
            &root.catalog,
            &effects(&[TRANSFER]),
            10,
        )
        .unwrap(),
        terminal_effects: committed,
    };
    assert!(record.is_live(4_999));
    assert!(!record.is_live(5_000));
    let release = terminal_effect_id(&log("release")).unwrap();
    let index = record.pending_effect(&release).expect("committed");
    assert_eq!(record.pending_effect(&[0; 32]), None);
    record.terminal_effects[index].performed = true;
    assert_eq!(
        record.pending_effect(&release),
        None,
        "a performed effect lends no capability again"
    );
    assert!(record.is_live(0), "the refund is still to be performed");
    for effect in &mut record.terminal_effects {
        effect.performed = true;
    }
    assert!(!record.is_live(0));

    let bytes = norito::encode_canonical(&record).unwrap();
    assert_eq!(
        norito::decode_canonical::<ObligationRecordV1>(&bytes).unwrap(),
        record
    );
    let json = norito::json::to_json(&record).unwrap();
    assert_eq!(
        norito::json::from_str::<ObligationRecordV1>(&json).unwrap(),
        record
    );

    let id = ObligationId::derive(&owner).unwrap();
    assert_eq!(ObligationId::derive(&owner), Ok(id));
    let another = AccountId::new(
        KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    assert_ne!(ObligationId::derive(&another), Ok(id));
    assert_eq!(id.to_string().len(), 64);
}

#[test]
fn a_migration_record_holds_its_barrier_until_the_cutover() {
    let mut record = MigrationRecordV1::started(40);
    assert_eq!(record.status, MigrationStatusV1::Running);
    assert!(record.barrier_holds() && !record.staged() && record.cursor.is_empty());
    for (status, barrier, staged) in [
        (MigrationStatusV1::Running, true, false),
        (MigrationStatusV1::Complete(55), true, true),
        (MigrationStatusV1::Faulted(50), true, false),
        (MigrationStatusV1::CutOver(60), false, true),
        (MigrationStatusV1::Drained(70), false, true),
        (MigrationStatusV1::StaleWrite(65), false, true),
        (MigrationStatusV1::Replaced(52), false, false),
    ] {
        record.status = status;
        assert_eq!(record.barrier_holds(), barrier, "{status:?}");
        assert_eq!(record.staged(), staged, "{status:?}");
    }

    record.cursor = vec![1, 2, 3];
    record.rows = 9;
    record.stages = 3;
    record.status = MigrationStatusV1::Faulted(50);
    let resumed = record.taken_over();
    assert_eq!(resumed.status, MigrationStatusV1::Running);
    assert_eq!(
        (
            resumed.started_at,
            resumed.cursor.as_slice(),
            resumed.rows,
            resumed.stages
        ),
        (40, &[1, 2, 3][..], 9, 3),
        "a replacement resumes from the committed cursor"
    );
    record.status = MigrationStatusV1::Complete(55);
    assert_eq!(record.taken_over().status, MigrationStatusV1::Complete(55));

    let bytes = norito::encode_canonical(&record).unwrap();
    assert_eq!(
        norito::decode_canonical::<MigrationRecordV1>(&bytes).unwrap(),
        record
    );
    let json = norito::json::to_json(&record).unwrap();
    assert_eq!(
        norito::json::from_str::<MigrationRecordV1>(&json).unwrap(),
        record
    );
}
