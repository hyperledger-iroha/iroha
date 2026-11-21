//! Descriptor selection, original envelope custody and deterministic refusal boundaries.

use std::collections::BTreeMap;

use iroha_data_model::{
    isi::{Log, OpaqueInstructionV1},
    level::Level,
    runtime::{
        carrier::{CarrierRecordV1, CarrierValueV1, carrier_schema_hash_v1, encode_carrier_v1},
        definition::{
            BindingV1, CarrierNodeV1, CarrierSchemaV1, ResourceLimitsV1, RestrictionEntryV1,
            RestrictionV1, StorageDescriptorV1,
        },
    },
};

use super::*;

const WIRE: &str = "test.generic.carrier.v1";
const HEIGHT: u64 = 10;

pub(super) struct Fixture {
    pub(super) selection: RootSelectionV1,
    pub(super) published: BTreeMap<DefinitionId, DefinitionDescriptorV1>,
    pub(super) instruction: InstructionBox,
    interface: DefinitionId,
    implementation: DefinitionId,
}

fn payload_schema() -> CarrierSchemaV1 {
    CarrierSchemaV1 {
        nodes: vec![CarrierNodeV1::U16],
    }
}

pub(super) fn fixture() -> Fixture {
    let catalog = super::super::release_catalog().unwrap();
    let profile = catalog.generic_carriers.unwrap();
    let mut selection = catalog.baseline.clone();
    selection.kernel = profile.kernel;
    selection.commitment_profile = profile.commitment;
    selection.installed_storage.extend([
        profile.backing,
        catalog.world_storage["world.runtime_generic_artifacts"],
    ]);
    selection.installed_storage.sort();
    selection.active.push(profile.host);
    let mut published = catalog.definitions.clone();
    let interface = DefinitionDescriptorV1::Interface(InterfaceDescriptorV1 {
        wire_id: WIRE.into(),
        payload: SchemaV1::Carrier(payload_schema()),
        rejection_codes: vec![],
        events: vec![],
        metering_class: profile::METERING_CLASS.into(),
        contract_statement: iroha_crypto::Hash::new(b"fixture carrier no-op").into(),
    });
    let interface_id = interface.id().unwrap();
    published.insert(interface_id, interface);
    let implementation = DefinitionDescriptorV1::Implementation(ImplementationDescriptorV1 {
        entrypoint: EntrypointV1::Instructions(vec![interface_id]),
        code: ImplementationCodeV1::Ivm {
            code_hash: iroha_crypto::Hash::new(b"fixture artifact not executed by decoding").into(),
            entrypoint: "main".into(),
        },
        reads: vec![],
        writes: vec![],
        scans: vec![],
        events: vec![],
        permissions: vec![],
        parameters: vec![],
        invokes: vec![],
        host_profile: Some(profile.host),
        services_profile: selection.services_profile,
        limits: ResourceLimitsV1 {
            max_gas: 100,
            max_effects: 1,
            max_call_depth: 1,
        },
    });
    let implementation_id = implementation.id().unwrap();
    published.insert(implementation_id, implementation);
    selection.active.extend([interface_id, implementation_id]);
    selection.active.sort();
    selection.bindings.push(BindingV1 {
        wire_id: WIRE.into(),
        interface: interface_id,
        implementation: implementation_id,
    });
    selection.bindings.sort_by(|a, b| a.wire_id.cmp(&b.wire_id));
    selection.check_closure(&published).unwrap();
    let frame = encode_carrier_v1(&payload_schema(), &CarrierValueV1::U16(256)).unwrap();
    Fixture {
        selection,
        published,
        instruction: OpaqueInstructionV1::new(WIRE, frame).unwrap().into(),
        interface: interface_id,
        implementation: implementation_id,
    }
}

fn decode(fixture: &Fixture) -> Result<Option<DecodedInstruction<'_>>, Error> {
    decode_selected(
        super::super::release_catalog().unwrap(),
        &fixture.selection,
        &fixture.published,
        HEIGHT,
        &fixture.instruction,
    )
}

fn replace_implementation(
    fixture: &mut Fixture,
    change: impl FnOnce(&mut ImplementationDescriptorV1),
) {
    let DefinitionDescriptorV1::Implementation(mut descriptor) =
        fixture.published[&fixture.implementation].clone()
    else {
        panic!("implementation")
    };
    change(&mut descriptor);
    let descriptor = DefinitionDescriptorV1::Implementation(descriptor);
    let id = descriptor.id().unwrap();
    fixture.published.insert(id, descriptor);
    let old = fixture.implementation;
    fixture.selection.active.retain(|current| *current != old);
    fixture.selection.active.push(id);
    fixture.selection.active.sort();
    fixture
        .selection
        .bindings
        .iter_mut()
        .find(|binding| binding.wire_id == WIRE)
        .unwrap()
        .implementation = id;
    fixture.implementation = id;
}

#[test]
fn decoding_borrows_exact_descriptors_and_envelope_without_granting_execution_support() {
    let fixture = fixture();
    let before = norito::encode_canonical(&fixture.instruction).unwrap();
    let decoded = decode(&fixture).unwrap().unwrap();
    assert_eq!(decoded.interface_id, fixture.interface);
    assert_eq!(decoded.implementation_id, fixture.implementation);
    assert_eq!(decoded.interface.wire_id, WIRE);
    assert_eq!(
        decoded.implementation.host_profile,
        super::super::release_catalog()
            .unwrap()
            .generic_carriers
            .map(|profile| profile.host)
    );
    assert_eq!(decoded.payload, [0, 1]);
    assert!(decoded.payload.as_ptr() >= decoded.frame.as_ptr());
    assert!(decoded.payload.as_ptr_range().end <= decoded.frame.as_ptr_range().end);
    assert!(std::ptr::eq(
        decoded.frame,
        fixture.instruction.as_opaque().unwrap().frame()
    ));
    assert_eq!(
        norito::encode_canonical(&fixture.instruction).unwrap(),
        before
    );
    assert_eq!(
        super::super::release_catalog()
            .unwrap()
            .unsupported(&fixture.selection),
        Some(fixture.selection.kernel)
    );
}

#[test]
fn baseline_and_typed_dispatch_keep_their_existing_opaque_rules() {
    let mut fixture = fixture();
    let catalog = super::super::release_catalog().unwrap();
    fixture.selection.kernel = catalog.baseline.kernel;
    assert!(decode(&fixture).unwrap().is_none());
    assert!(matches!(
        super::super::envelope::opaque_rejection(&fixture.instruction),
        Some(RuntimeRejectionV1::MalformedPayload { .. })
    ));
    fixture.selection.kernel = catalog.generic_carriers.unwrap().kernel;
    fixture.instruction = Log::new(Level::INFO, "native remains native".into()).into();
    assert!(decode(&fixture).unwrap().is_none());
}

#[test]
fn missing_bindings_and_restrictions_precede_any_payload_interpretation() {
    let mut fixture = fixture();
    fixture.instruction = OpaqueInstructionV1::new(WIRE, vec![0]).unwrap().into();
    fixture
        .selection
        .bindings
        .retain(|binding| binding.wire_id != WIRE);
    assert!(matches!(
        decode(&fixture),
        Err(Error::Rejected(InstructionExecutionError::Runtime(
            RuntimeRejectionV1::DefinitionInactive { .. }
        )))
    ));
    let mut fixture = self::fixture();
    fixture.instruction = OpaqueInstructionV1::new(WIRE, vec![0]).unwrap().into();
    let host = super::super::release_catalog()
        .unwrap()
        .generic_carriers
        .unwrap()
        .host;
    for id in [fixture.interface, fixture.implementation, host] {
        fixture.selection.restrictions = vec![RestrictionEntryV1 {
            definition: id,
            restriction: RestrictionV1::AdmissionClosed(HEIGHT),
        }];
        assert!(
            matches!(decode(&fixture), Err(Error::Rejected(InstructionExecutionError::Runtime(RuntimeRejectionV1::DefinitionRestricted { definition, .. }))) if definition == id)
        );
    }
}

#[test]
fn wrong_host_and_undeclared_effect_families_never_reach_the_decoder() {
    for change in [
        |implementation: &mut ImplementationDescriptorV1| {
            implementation.host_profile = Some(DefinitionId([3; 32]))
        },
        |implementation: &mut ImplementationDescriptorV1| {
            implementation.events.push(DefinitionId([3; 32]))
        },
        |implementation: &mut ImplementationDescriptorV1| {
            implementation.permissions.push(DefinitionId([3; 32]))
        },
        |implementation: &mut ImplementationDescriptorV1| {
            implementation.parameters.push(DefinitionId([3; 32]))
        },
        |implementation: &mut ImplementationDescriptorV1| {
            implementation.invokes.push(DefinitionId([3; 32]))
        },
        |implementation: &mut ImplementationDescriptorV1| {
            implementation.scans.push(DefinitionId([3; 32]))
        },
        |implementation: &mut ImplementationDescriptorV1| {
            implementation.limits.max_gas = profile::MAX_GAS + 1
        },
        |implementation: &mut ImplementationDescriptorV1| {
            implementation.limits.max_effects = profile::MAX_EFFECTS + 1
        },
        |implementation: &mut ImplementationDescriptorV1| implementation.limits.max_call_depth = 2,
    ] {
        let mut fixture = fixture();
        replace_implementation(&mut fixture, change);
        assert!(matches!(decode(&fixture), Err(Error::Contract(_))));
    }
}

#[test]
fn descriptor_accepts_profile_maxima_and_bounds_reads_and_writes_independently() {
    let mut fixture = fixture();
    let mut owners = Vec::new();
    for index in 0..17 {
        let storage = DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
            namespace: format!("test.generic.bound.{index}"),
            shape: StorageShapeV1::Table {
                key: SchemaV1::Carrier(payload_schema()),
            },
            value: SchemaV1::Carrier(payload_schema()),
            initial: None,
            invariants: iroha_crypto::Hash::new(b"fixture table").into(),
            projection: ProjectionV1::BareNorito,
        });
        let id = storage.id().unwrap();
        fixture.published.insert(id, storage);
        fixture.selection.installed_storage.push(id);
        owners.push(id);
    }
    fixture.selection.installed_storage.sort();
    owners.sort();
    replace_implementation(&mut fixture, |descriptor| {
        descriptor.reads = owners[..16].to_vec();
        descriptor.writes = owners[..16].to_vec();
        descriptor.limits.max_gas = 10_000_000;
        descriptor.limits.max_effects = 64;
    });
    fixture.selection.check_closure(&fixture.published).unwrap();
    assert!(decode(&fixture).unwrap().is_some());

    for (read_count, write_count) in [(17, 16), (16, 17)] {
        replace_implementation(&mut fixture, |descriptor| {
            descriptor.reads = owners[..read_count].to_vec();
            descriptor.writes = owners[..write_count].to_vec();
        });
        assert!(matches!(
            decode(&fixture),
            Err(Error::Contract(
                "carrier storage dependencies exceed the canonical bounded list"
            ))
        ));
    }
}

#[test]
fn storage_dependencies_require_installed_carrier_tables_and_no_semantic_projection() {
    let mut fixture = fixture();
    let storage = DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
        namespace: "test.generic.table.v1".into(),
        shape: StorageShapeV1::Table {
            key: SchemaV1::Carrier(payload_schema()),
        },
        value: SchemaV1::Carrier(payload_schema()),
        initial: None,
        invariants: iroha_crypto::Hash::new(b"fixture table").into(),
        projection: ProjectionV1::BareNorito,
    });
    let storage_id = storage.id().unwrap();
    fixture.published.insert(storage_id, storage.clone());
    replace_implementation(&mut fixture, |implementation| {
        implementation.reads = vec![storage_id];
        implementation.writes = vec![storage_id];
    });
    assert!(matches!(
        decode(&fixture),
        Err(Error::Contract("storage is not installed"))
    ));
    fixture.selection.installed_storage.push(storage_id);
    fixture.selection.installed_storage.sort();
    fixture.selection.check_closure(&fixture.published).unwrap();
    assert!(decode(&fixture).unwrap().is_some());
    fixture.selection.restrictions.push(RestrictionEntryV1 {
        definition: storage_id,
        restriction: RestrictionV1::Disabled(HEIGHT),
    });
    assert!(
        matches!(decode(&fixture), Err(Error::Rejected(InstructionExecutionError::Runtime(RuntimeRejectionV1::DefinitionRestricted { definition, .. }))) if definition == storage_id)
    );
    fixture.selection.restrictions.clear();
    let DefinitionDescriptorV1::Storage(mut changed) = storage else {
        unreachable!()
    };
    changed.projection = ProjectionV1::Semantic("native.projection".into());
    let changed = DefinitionDescriptorV1::Storage(changed);
    let changed_id = changed.id().unwrap();
    fixture.published.insert(changed_id, changed);
    fixture
        .selection
        .installed_storage
        .retain(|id| *id != storage_id);
    fixture.selection.installed_storage.push(changed_id);
    fixture.selection.installed_storage.sort();
    replace_implementation(&mut fixture, |implementation| {
        implementation.reads = vec![changed_id];
        implementation.writes = vec![changed_id];
    });
    assert!(matches!(decode(&fixture), Err(Error::Contract(_))));
}

#[test]
fn malformed_and_dense_frames_are_stable_rejections_but_local_refusals_stay_original() {
    let mut fixture = fixture();
    let original = fixture.instruction.clone();
    for bytes in [
        vec![0],
        norito::encode_canonical(&CarrierRecordV1 {
            schema_hash: carrier_schema_hash_v1(&payload_schema()).unwrap(),
            payload: vec![1],
        })
        .unwrap(),
    ] {
        fixture.instruction = OpaqueInstructionV1::new(WIRE, bytes).unwrap().into();
        assert!(matches!(
            decode(&fixture),
            Err(Error::Rejected(InstructionExecutionError::Runtime(
                RuntimeRejectionV1::MalformedPayload { .. }
            )))
        ));
    }
    fixture.instruction = OpaqueInstructionV1::new_dense(WIRE, vec![0, 1])
        .unwrap()
        .into();
    assert!(matches!(
        decode(&fixture),
        Err(Error::Rejected(InstructionExecutionError::Runtime(
            RuntimeRejectionV1::MalformedPayload { .. }
        )))
    ));
    fixture.instruction = original;
    // Borrowing avoids materializing a value graph, but Norito's framed-field reads
    // still debit the original cumulative allocation counter. A zero counter must
    // therefore retain the original refusal for the first 32-byte schema-hash field.
    let schema = payload_schema();
    let frame = fixture.instruction.as_opaque().unwrap().frame();
    assert_eq!(
        validated_carrier_payload_v1(&schema, frame).unwrap(),
        [0, 1]
    );
    let no_allocation = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64);
    let result = norito::with_decode_limits_scope(no_allocation, || decode(&fixture));
    let Err(Error::Codec(original)) = result else {
        panic!("borrowed field validation must keep its original counter refusal");
    };
    assert_eq!(original.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    assert!(matches!(
        original.into_local_refusal().unwrap().resource_error(),
        norito::core::DecodeResourceError::TotalAllocationExceeded { limit: 0, .. }
    ));
    let limits = norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, 64);
    norito::with_decode_limits_scope(limits, || {
        assert!(
            matches!(decode(&fixture), Err(Error::Codec(error)) if error.kind() == DecodeAttemptErrorKind::EnclosingLimit)
        );
    });
    assert!(decode(&fixture).unwrap().is_some());
}

#[test]
fn invocation_requires_exact_installed_artifact_storage() {
    let catalog = super::super::release_catalog().unwrap();
    let artifacts = catalog.world_storage["world.runtime_generic_artifacts"];
    let mut original = fixture();
    original
        .selection
        .installed_storage
        .retain(|id| id != &artifacts);
    assert!(matches!(
        decode(&original),
        Err(Error::Contract(
            "carrier invocation requires its exact installed artifact storage"
        ))
    ));
    let mut original = fixture();
    original.published.remove(&artifacts);
    assert!(matches!(
        decode(&original),
        Err(Error::Contract(
            "carrier invocation requires its exact installed artifact storage"
        ))
    ));
    let mut original = fixture();
    original
        .published
        .insert(artifacts, original.published[&original.interface].clone());
    assert!(matches!(
        decode(&original),
        Err(Error::Contract(
            "carrier invocation requires its exact installed artifact storage"
        ))
    ));
}
