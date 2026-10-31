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
    iroha_data_model::isi::wire_id_of::<Log>().expect("registered")
}

#[test]
fn the_baseline_resolves_every_built_in_instruction() {
    let selection = baseline();
    assert_eq!(
        resolve(&log(), &selection, 1, || None),
        Ok(Some(selection.bindings[0].implementation)),
        "the bound implementation executes it"
    );
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
            Ok(Some(binding.implementation)),
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
        Ok(Some(binding.implementation)),
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

#[test]
fn a_node_drafts_the_activation_of_its_release_additions() {
    use crate::state::WorldReadOnly as _;
    use catalog::{
        additions::test_additions::{LOG, LOG_MODULE, LOG_WIRE_ID},
        test_catalog,
    };
    let network = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0x61; 32])),
    );
    test_catalog::with_additions(&[LOG], || {
        let state = crate::state::State::new_for_testing(
            crate::state::World::default(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let view = state.world_view();
        let change = draft_addition(&view, network, LOG_MODULE, 10, [1; 32]).expect("draft");
        let catalog = release_catalog().expect("test catalog");
        assert_eq!(change.predecessor, catalog.baseline_id);
        assert_eq!(change.network, network);
        assert_eq!(change.effective_height, 10);
        let applied = change
            .apply(
                &catalog.baseline,
                &change_set::PublishedDefinitions(view.runtime_definitions()),
            )
            .expect("the draft applies to the committed selection");
        assert!(
            applied
                .selection
                .bindings
                .iter()
                .any(|binding| binding.wire_id == LOG_WIRE_ID)
        );
        assert!(
            draft_addition(&view, network, "iroha.test.unknown", 10, [1; 32]).is_err(),
            "only additions of this release are drafted"
        );
    });
}
