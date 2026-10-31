//! Deferred closures remain bound to their original semantics.

use super::*;
use iroha_data_model::runtime::definition::{EntrypointV1, RestrictionEntryV1, RestrictionV1};

fn baseline() -> (
    &'static super::super::catalog::ReleaseCatalog,
    RootSelectionV1,
) {
    let catalog = super::super::release_catalog().unwrap();
    (catalog, catalog.baseline.clone())
}

fn add_unrelated_interface(
    selection: &mut RootSelectionV1,
    definitions: &mut BTreeMap<DefinitionId, DefinitionDescriptorV1>,
) {
    let original = selection.bindings[0].clone();
    let mut interface = definitions[&original.interface].clone();
    let DefinitionDescriptorV1::Interface(descriptor) = &mut interface else {
        panic!("interface")
    };
    descriptor.wire_id = "zz.unrelated.v1".into();
    let interface_id = interface.id().unwrap();
    let mut implementation = definitions[&original.implementation].clone();
    let DefinitionDescriptorV1::Implementation(descriptor) = &mut implementation else {
        panic!("implementation")
    };
    descriptor.entrypoint = EntrypointV1::Instructions(vec![interface_id]);
    descriptor.invokes.clear();
    let implementation_id = implementation.id().unwrap();
    definitions.insert(interface_id, interface);
    definitions.insert(implementation_id, implementation);
    selection.active.extend([interface_id, implementation_id]);
    selection.active.sort();
    selection.bindings.push(BindingV1 {
        wire_id: "zz.unrelated.v1".into(),
        interface: interface_id,
        implementation: implementation_id,
    });
    selection.check_closure(definitions).unwrap();
}

#[test]
fn baseline_calls_capture_cycles_once_and_ignore_unrelated_additions() {
    let (catalog, selection) = baseline();
    let wire = selection.bindings[0].wire_id.as_str();
    let expected = capture(
        &selection,
        &catalog.definitions,
        [Some(wire)].into_iter(),
        false,
    )
    .unwrap();
    assert_eq!(
        expected.bindings, selection.bindings,
        "the conservative baseline invokes every baseline interface"
    );
    assert_eq!(check_selection(&expected, &selection, 7), Ok(()));
    let mut added = selection.clone();
    let mut definitions = catalog.definitions.clone();
    add_unrelated_interface(&mut added, &mut definitions);
    assert_eq!(check_selection(&expected, &added, 7), Ok(()));
    assert_eq!(
        capture(&added, &definitions, [Some(wire)].into_iter(), false).unwrap(),
        expected
    );
    assert_eq!(
        digest(
            &expected,
            crate::executor::Executor::Initial.runtime_consent_hash()
        )
        .unwrap(),
        digest(
            &expected.clone(),
            crate::executor::Executor::Initial.runtime_consent_hash()
        )
        .unwrap()
    );
}

#[test]
fn a_rebinding_of_a_transitive_call_requires_new_consent() {
    let (catalog, selection) = baseline();
    let expected = capture(
        &selection,
        &catalog.definitions,
        [Some(selection.bindings[0].wire_id.as_str())].into_iter(),
        false,
    )
    .unwrap();
    let mut changed = selection;
    changed.bindings.last_mut().unwrap().implementation = DefinitionId([0x7f; 32]);
    assert!(matches!(
        check_selection(&expected, &changed, 7),
        Err(Error::Runtime(
            RuntimeRejectionV1::ExpectationMismatch { .. }
        ))
    ));
}

#[test]
fn restrictions_on_any_captured_dependency_suspend_deferred_execution() {
    let (catalog, selection) = baseline();
    let expected = capture(
        &selection,
        &catalog.definitions,
        [Some(selection.bindings[0].wire_id.as_str())].into_iter(),
        false,
    )
    .unwrap();
    let definition = *expected.definitions.last().unwrap();
    for restriction in [
        RestrictionV1::AdmissionClosed(7),
        RestrictionV1::CompletionOnly(7),
        RestrictionV1::Disabled(7),
    ] {
        let mut changed = selection.clone();
        changed.restrictions = vec![RestrictionEntryV1 {
            definition,
            restriction,
        }];
        assert_eq!(check_selection(&expected, &changed, 6), Ok(()));
        assert_eq!(
            check_selection(&expected, &changed, 7),
            Err(Error::Runtime(RuntimeRejectionV1::DefinitionRestricted {
                definition,
                restriction
            }))
        );
    }
}

#[test]
fn opaque_code_requires_renewed_consent_for_expanded_callable_surface() {
    let (catalog, mut selection) = baseline();
    let expected = capture(&selection, &catalog.definitions, std::iter::empty(), true).unwrap();
    add_unrelated_interface(&mut selection, &mut catalog.definitions.clone());
    assert!(matches!(
        check_selection(&expected, &selection, 7),
        Err(Error::Runtime(
            RuntimeRejectionV1::ExpectationMismatch { .. }
        ))
    ));
}

#[test]
fn inactive_or_private_primitive_cannot_be_captured_as_deferred_entrypoint() {
    let (catalog, selection) = baseline();
    assert!(matches!(
        capture(
            &selection,
            &catalog.definitions,
            [Some("unknown")].into_iter(),
            false
        ),
        Err(Error::Runtime(
            RuntimeRejectionV1::DefinitionInactive { .. }
        ))
    ));
    assert!(matches!(
        capture(&selection, &catalog.definitions, [None].into_iter(), false),
        Err(Error::Runtime(
            RuntimeRejectionV1::ExpectationMismatch { .. }
        ))
    ));
}
