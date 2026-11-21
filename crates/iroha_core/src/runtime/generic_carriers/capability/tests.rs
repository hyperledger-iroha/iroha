//! Complete selected-declaration census without activation or artifact authority.

use super::*;
use crate::runtime::generic_carriers::tests::{Fixture, fixture};
use iroha_data_model::runtime::definition::{CarrierNodeV1, CarrierSchemaV1, StorageDescriptorV1};

fn check(fixture: &Fixture) -> Result<CarrierCapability<'_>, Error> {
    validate_compiled_selection(
        crate::runtime::release_catalog().unwrap(),
        &fixture.selection,
        &fixture.published,
    )
}

fn install(fixture: &mut Fixture, definition: DefinitionDescriptorV1) -> DefinitionId {
    let id = definition.id().unwrap();
    let is_storage = matches!(definition, DefinitionDescriptorV1::Storage(_));
    fixture.published.insert(id, definition);
    let selected = if is_storage {
        &mut fixture.selection.installed_storage
    } else {
        &mut fixture.selection.active
    };
    selected.push(id);
    selected.sort();
    id
}

fn table(namespace: &str) -> DefinitionDescriptorV1 {
    let schema = SchemaV1::Carrier(CarrierSchemaV1 {
        nodes: vec![CarrierNodeV1::U16],
    });
    DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
        namespace: namespace.into(),
        shape: StorageShapeV1::Table {
            key: schema.clone(),
        },
        value: schema,
        initial: None,
        invariants: [9; 32],
        projection: ProjectionV1::BareNorito,
    })
}

#[test]
fn exact_dynamic_declarations_are_observed_without_claiming_qualification() {
    let catalog = crate::runtime::release_catalog().unwrap();
    let mut original = fixture();
    install(&mut original, table("test.carrier.capability.rows"));
    original
        .selection
        .check_closure(&original.published)
        .unwrap();
    assert!(check(&original).is_ok());
    assert!(catalog.unsupported(&original.selection).is_some());
    assert!(matches!(
        crate::runtime::attestation::release_support(catalog, &original.selection),
        crate::runtime::attestation::ReleaseSupport::Missing(_)
    ));
    // No artifact bytes exist in this lookup; only the installed artifact owner's exact
    // descriptor is checked. A malformed unselected publication grants no extra capability.
    original
        .published
        .insert(DefinitionId([0; 32]), table("not a valid namespace"));
    assert!(check(&original).is_ok());
    assert!(validate_compiled_selection(catalog, &catalog.baseline, &catalog.definitions).is_err());
}

#[test]
fn every_selected_descriptor_keeps_exact_identity_and_storage_shape() {
    let mut original = fixture();
    let id = install(&mut original, table("test.carrier.capability.rows"));
    assert!(check(&original).is_ok());
    original.published.remove(&id);
    assert!(check(&original).is_err());
    original
        .published
        .insert(id, table("test.carrier.capability.substituted"));
    assert!(check(&original).is_err());
    let mut original = fixture();
    let mut cell = table("test.carrier.capability.cell");
    let DefinitionDescriptorV1::Storage(storage) = &mut cell else {
        unreachable!()
    };
    storage.shape = StorageShapeV1::Cell;
    install(&mut original, cell);
    assert!(check(&original).is_err());
    let mut original = fixture();
    install(&mut original, table("world.runtime_generic_artifacts"));
    assert!(check(&original).is_err());
    let mut original = fixture();
    let first = install(&mut original, table("test.carrier.capability.overlap"));
    let mut second = original.published[&first].clone();
    let DefinitionDescriptorV1::Storage(storage) = &mut second else {
        unreachable!()
    };
    storage.invariants[0] ^= 1;
    install(&mut original, second);
    assert!(check(&original).is_err());
}

#[test]
fn dynamic_interface_cannot_claim_a_native_registry_wire() {
    let mut original = fixture();
    let binding = original
        .selection
        .bindings
        .iter()
        .find(|binding| binding.wire_id == "test.generic.carrier.v1")
        .unwrap()
        .clone();
    let DefinitionDescriptorV1::Interface(mut interface) =
        original.published[&binding.interface].clone()
    else {
        unreachable!()
    };
    interface.wire_id = iroha_data_model::isi::registry::built_in_instruction_schemas()
        .next()
        .unwrap()
        .wire_id
        .into();
    let wire = interface.wire_id.clone();
    let interface_id = install(&mut original, DefinitionDescriptorV1::Interface(interface));
    let DefinitionDescriptorV1::Implementation(mut implementation) =
        original.published[&binding.implementation].clone()
    else {
        unreachable!()
    };
    implementation.entrypoint = EntrypointV1::Instructions(vec![interface_id]);
    let implementation_id = install(
        &mut original,
        DefinitionDescriptorV1::Implementation(implementation),
    );
    original
        .selection
        .bindings
        .retain(|candidate| candidate.wire_id != wire && candidate.wire_id != binding.wire_id);
    original
        .selection
        .bindings
        .push(iroha_data_model::runtime::definition::BindingV1 {
            wire_id: wire,
            interface: interface_id,
            implementation: implementation_id,
        });
    original
        .selection
        .bindings
        .sort_by(|a, b| a.wire_id.cmp(&b.wire_id));
    original
        .selection
        .active
        .retain(|id| *id != binding.interface && *id != binding.implementation);
    assert!(matches!(
        check(&original),
        Err(Error::Contract(
            "carrier interface collides with a registered native wire ID or has invalid syntax"
        ))
    ));
}

#[test]
fn unbound_replacement_implementation_is_still_checked_when_selected() {
    let mut original = fixture();
    let id = original
        .selection
        .bindings
        .iter()
        .find(|binding| binding.wire_id == "test.generic.carrier.v1")
        .unwrap()
        .implementation;
    let DefinitionDescriptorV1::Implementation(mut implementation) =
        original.published[&id].clone()
    else {
        unreachable!()
    };
    implementation.limits.max_effects = profile::MAX_EFFECTS + 1;
    let unbound = install(
        &mut original,
        DefinitionDescriptorV1::Implementation(implementation),
    );
    assert!(
        !original
            .selection
            .bindings
            .iter()
            .any(|binding| binding.implementation == unbound)
    );
    assert!(matches!(
        check(&original),
        Err(Error::Contract(
            "carrier resource limits exceed the restricted profile"
        ))
    ));
}

#[test]
fn original_codec_refusal_is_not_a_missing_definition_verdict() {
    let original = fixture();
    // Streaming identity/schema serialization keeps the enclosing encoder's first sticky
    // refusal. The capability layer forwards the codec error, never Missing or success.
    let mut forwarded = false;
    let result = norito::core::with_bounded_encode_work(
        norito::core::EncodeWorkLimits {
            max_frame_bytes: usize::MAX,
            max_byte_visits: usize::MAX,
            max_operations: 0,
            max_depth: usize::MAX,
        },
        || match check(&original) {
            Err(Error::Codec(original)) => {
                forwarded = true;
                Err(original.into_error())
            }
            Err(other) => panic!("original serializer error was replaced: {other}"),
            Ok(_) => Ok(()),
        },
    );
    assert!(forwarded);
    assert!(matches!(
        result,
        Err(norito::core::BoundedEncodeWorkError::Refused(
            norito::core::EncodeWorkRefusal::Operations
        ))
    ));
}
