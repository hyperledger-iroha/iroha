//! Execution resolves every instruction against the root's selection.

use iroha_data_model::{
    isi::{InstructionBox, Log, error::InstructionExecutionError},
    level::Level,
    runtime::definition::RestrictionEntryV1,
};

use super::*;

fn log() -> InstructionBox {
    Log::new(Level::INFO, "resolved".into()).into()
}

fn baseline() -> RootSelectionV1 {
    founded_selection().expect("baseline")
}

fn wire_id() -> &'static str {
    iroha_data_model::isi::instruction_wire_id(&log()).expect("registered")
}

#[test]
fn the_baseline_resolves_every_built_in_instruction() {
    let selection = baseline();
    assert_eq!(resolve(&log(), &selection, 1, || None), Ok(()));
    assert_eq!(selection.bindings.len(), {
        iroha_data_model::isi::registry::built_in_instruction_schemas().count()
    });
}

#[test]
fn an_unbound_wire_id_is_inactive() {
    let mut selection = baseline();
    selection
        .bindings
        .retain(|binding| binding.wire_id != wire_id());
    assert_eq!(
        resolve(&log(), &selection, 1, || None),
        Err(InstructionExecutionError::Runtime(
            RuntimeRejectionV1::DefinitionInactive {
                wire_id: wire_id().to_owned(),
            }
        ))
    );
}

#[test]
fn restrictions_stop_new_work_from_their_height() {
    let selection = baseline();
    let binding = selection
        .bindings
        .iter()
        .find(|binding| binding.wire_id == wire_id())
        .expect("bound")
        .clone();
    for (definition, restriction) in [
        (binding.interface, RestrictionV1::Disabled(5)),
        (binding.implementation, RestrictionV1::AdmissionClosed(5)),
        (binding.implementation, RestrictionV1::CompletionOnly(5)),
    ] {
        let mut restricted = selection.clone();
        restricted.restrictions = vec![RestrictionEntryV1 {
            definition,
            restriction,
        }];
        assert_eq!(
            resolve(&log(), &restricted, 4, || None),
            Ok(()),
            "before its height"
        );
        assert_eq!(
            resolve(&log(), &restricted, 5, || None),
            Err(InstructionExecutionError::Runtime(
                RuntimeRejectionV1::DefinitionRestricted {
                    definition,
                    restriction,
                }
            ))
        );
    }

    let mut deprecated = selection;
    deprecated.restrictions = vec![RestrictionEntryV1 {
        definition: binding.implementation,
        restriction: RestrictionV1::Deprecated(1),
    }];
    assert_eq!(
        resolve(&log(), &deprecated, 9, || None),
        Ok(()),
        "deprecation keeps execution"
    );
}

#[test]
fn a_live_obligation_continues_or_completes_through_its_restriction() {
    use obligations::Step;
    let selection = baseline();
    let binding = selection
        .bindings
        .iter()
        .find(|binding| binding.wire_id == wire_id())
        .expect("bound")
        .clone();
    let restricted = |restriction| {
        let mut restricted = selection.clone();
        restricted.restrictions = vec![RestrictionEntryV1 {
            definition: binding.interface,
            restriction,
        }];
        restricted
    };
    for (restriction, step, runs) in [
        (
            RestrictionV1::AdmissionClosed(5),
            Some(Step::Continue),
            true,
        ),
        (
            RestrictionV1::AdmissionClosed(5),
            Some(Step::Complete),
            true,
        ),
        (RestrictionV1::AdmissionClosed(5), None, false),
        (
            RestrictionV1::CompletionOnly(5),
            Some(Step::Continue),
            false,
        ),
        (RestrictionV1::CompletionOnly(5), Some(Step::Complete), true),
        (RestrictionV1::Disabled(5), Some(Step::Complete), false),
    ] {
        let result = resolve(&log(), &restricted(restriction), 5, || step);
        assert_eq!(result.is_ok(), runs, "{restriction:?} {step:?}");
    }
    let mut asked = 0;
    resolve(&log(), &selection, 5, || {
        asked += 1;
        None
    })
    .expect("unrestricted");
    assert_eq!(asked, 0, "an unrestricted binding never reads obligations");
}

#[test]
fn a_new_world_is_founded_on_the_baseline() {
    let world = crate::state::World::default();
    let catalog = release_catalog().expect("catalog");
    assert_eq!(*world.runtime_selection.view().get(), catalog.baseline);
    let definitions = world.runtime_definitions.view();
    assert_eq!(
        mv::storage::StorageReadOnly::iter(&definitions).count(),
        catalog.baseline_definitions.len()
    );
    for id in &catalog.baseline_definitions {
        assert_eq!(
            mv::storage::StorageReadOnly::get(&definitions, id),
            catalog.definitions.get(id)
        );
    }
}
