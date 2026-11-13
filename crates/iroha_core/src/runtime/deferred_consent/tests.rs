//! Deferred-consent activation, immutable captures, suspension and explicit opt-in.

use super::*;
use crate::{
    runtime::{
        activate_additions_for_testing, catalog::additions::DEFERRED_CONSENT, resolve_executing,
    },
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_data_model::{
    block::BlockHeader,
    events::execute_trigger::ExecuteTriggerEventFilter,
    isi::{Log, Register, runtime_deferred::RegisterCurrentBindingTriggerV1},
    level::Level,
    runtime::definition::{RestrictionEntryV1, RestrictionV1},
    trigger::{
        Trigger,
        action::{Action, Repeats},
    },
};
use iroha_test_samples::ALICE_ID;

fn state(selected: bool) -> State {
    let state = State::new_for_testing(
        World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    if selected {
        activate_additions_for_testing(&state.world, DEFERRED_CONSENT.module).unwrap();
    }
    state
}

fn header() -> BlockHeader {
    BlockHeader::new(std::num::NonZeroU64::new(1).unwrap(), None, None, 1_000, 0)
}

fn log() -> InstructionBox {
    Log::new(Level::INFO, "deferred".to_owned()).into()
}

fn register_multisig(
    tx: &mut StateTransaction<'_, '_>,
    owner: &AccountId,
    proposal: &[u8; 32],
    instructions: &[InstructionBox],
    current_bindings: bool,
) -> Result<(), Error> {
    if let Some(record) = prepare_multisig(tx, owner, instructions, current_bindings)? {
        tx.world
            .runtime_deferred_consents
            .insert(multisig_key(owner, proposal), record);
    }
    Ok(())
}

fn trigger(name: &str) -> Trigger {
    let id: TriggerId = name.parse().unwrap();
    Trigger::new(
        id.clone(),
        Action::new(
            vec![log()],
            Repeats::Exactly(2),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(id)
                .under_authority(ALICE_ID.clone()),
        )
        .unwrap(),
    )
}

fn log_binding(tx: &StateTransaction<'_, '_>) -> iroha_data_model::runtime::definition::BindingV1 {
    let wire = iroha_data_model::isi::instruction_wire_id(&log())
        .unwrap()
        .to_owned();
    tx.world
        .runtime_selection
        .get()
        .bindings
        .iter()
        .find(|entry| entry.wire_id == wire)
        .unwrap()
        .clone()
}

#[test]
fn inactive_roots_do_not_capture_and_activation_requires_old_work_to_drain() {
    let state = state(false);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let before = tx.world.runtime_selection.get().clone();
    Register::trigger(trigger("old"))
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    assert!(tx.world.runtime_deferred_consents.iter().next().is_none());
    register_multisig(&mut tx, &ALICE_ID, &[1; 32], &[log()], false).unwrap();
    assert!(tx.world.runtime_deferred_consents.iter().next().is_none());
    let catalog = super::super::release_catalog().unwrap();
    let network = tx.network_id;
    let change = catalog
        .activation(
            DEFERRED_CONSENT.module,
            &before,
            &catalog.definitions,
            network,
            10,
            [1; 32],
        )
        .unwrap();
    let after = change
        .apply(&before, &catalog.definitions)
        .unwrap()
        .selection;
    assert!(
        check_transition(&*tx.world, &before, &after)
            .unwrap_err()
            .contains("drain")
    );
    tx.world.triggers.remove(&"old".parse().unwrap());
    assert_eq!(check_transition(&*tx.world, &before, &after), Ok(()));
    tx.world
        .smart_contract_state
        .insert("multisig/proposal/retained".parse().unwrap(), vec![0]);
    assert!(check_transition(&*tx.world, &before, &after).is_err());
}

#[test]
fn inactive_opt_in_fails_before_registering_a_trigger() {
    let state = state(false);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let trigger = trigger("inactive");
    let id = trigger.id().clone();
    assert!(
        RegisterCurrentBindingTriggerV1 { trigger }
            .execute(&ALICE_ID, &mut tx)
            .is_err()
    );
    assert!(tx.world.triggers.inspect_by_id(&id, |_| ()).is_none());
    assert!(tx.world.runtime_deferred_consents.iter().next().is_none());
}

#[test]
fn rejected_capture_leaves_no_trigger_or_consent_when_caller_retains_the_overlay() {
    let state = state(true);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let binding = log_binding(&tx);
    tx.world
        .runtime_selection
        .get_mut()
        .restrictions
        .push(RestrictionEntryV1 {
            definition: binding.interface,
            restriction: RestrictionV1::Disabled(1),
        });
    let trigger = trigger("refused_capture");
    let id = trigger.id().clone();
    assert!(
        Register::trigger(trigger.clone())
            .execute(&ALICE_ID, &mut tx)
            .is_err()
    );
    assert!(tx.world.triggers.inspect_by_id(&id, |_| ()).is_none());
    assert!(tx.world.runtime_deferred_consents.iter().next().is_none());
    RegisterCurrentBindingTriggerV1 { trigger }
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    assert!(trigger_ready_in_transaction(&tx, &id));
    assert!(resolve_executing(&tx, &log()).is_err());
}

#[test]
fn consent_cannot_move_between_authorities_or_trigger_bodies() {
    let state = state(true);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let trigger = trigger("identity");
    let id = trigger.id().clone();
    Register::trigger(trigger)
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    let key = Key::Trigger(id.clone());
    let original = tx
        .world
        .runtime_deferred_consents
        .get(&key)
        .unwrap()
        .clone();
    let mut changed = original.clone();
    changed.authority = iroha_test_samples::BOB_ID.clone();
    tx.world
        .runtime_deferred_consents
        .insert(key.clone(), changed);
    assert!(!trigger_ready(&*tx.world, &id, 1));
    let mut changed = original.clone();
    changed.executable = [0; 32];
    tx.world
        .runtime_deferred_consents
        .insert(key.clone(), changed);
    assert!(!trigger_ready(&*tx.world, &id, 1));
    tx.world.runtime_deferred_consents.insert(key, original);
    assert!(trigger_ready(&*tx.world, &id, 1));
}

#[test]
fn populated_consent_table_roundtrips_with_bounded_snapshot_bytes_and_undo() {
    let key = multisig_key(&ALICE_ID, &[4; 32]);
    let record = Record {
        authority: ALICE_ID.clone(),
        executable: [5; 32],
        consent: Consent::CurrentBindings,
    };
    let storage = mv::storage::Storage::<Key, Record>::new();
    {
        let mut block = storage.block();
        block.insert(key.clone(), record.clone());
        block.commit();
    }
    let json = norito::json::to_json(&storage).unwrap();
    assert_eq!(
        norito::json::to_json_bounded(&storage, json.len()).unwrap(),
        json
    );
    assert!(matches!(
        norito::json::to_json_bounded(&storage, json.len() - 1),
        Err(norito::json::BoundedJsonError::BodyTooLarge)
    ));
    let restored: mv::storage::Storage<Key, Record> = norito::json::from_str(&json).unwrap();
    assert_eq!(restored.view().get(&key), Some(&record));
    assert!(restored.block_and_revert().get(&key).is_none());
}

#[test]
fn service_and_consent_storage_are_selected_atomically() {
    let state = state(true);
    let mut block = state.block(header());
    let tx = block.transaction();
    let before = tx.world.runtime_selection.get().clone();
    let mut after = before.clone();
    let storage =
        super::super::release_catalog().unwrap().world_storage["world.runtime_deferred_consents"];
    after.installed_storage.retain(|id| *id != storage);
    assert!(check_transition(&*tx.world, &before, &after).is_err());
}

#[test]
fn default_trigger_capture_suspends_after_rebinding_or_restriction() {
    let state = state(true);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let trigger = trigger("captured");
    let id = trigger.id().clone();
    Register::trigger(trigger)
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    assert!(trigger_ready(&*tx.world, &id, 1));
    let original = tx.world.runtime_selection.get().clone();
    let binding = log_binding(&tx);
    tx.world
        .runtime_selection
        .get_mut()
        .bindings
        .iter_mut()
        .find(|item| item.wire_id == binding.wire_id)
        .unwrap()
        .implementation = original.kernel;
    assert!(!trigger_ready(&*tx.world, &id, 1));
    *tx.world.runtime_selection.get_mut() = original;
    tx.world.runtime_selection.get_mut().restrictions = vec![RestrictionEntryV1 {
        definition: binding.interface,
        restriction: RestrictionV1::CompletionOnly(2),
    }];
    assert!(trigger_ready(&*tx.world, &id, 1));
    assert!(!trigger_ready(&*tx.world, &id, 2));
}

#[test]
fn explicit_current_bindings_still_obey_execution_restrictions() {
    let state = state(true);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let trigger = trigger("current");
    let id = trigger.id().clone();
    RegisterCurrentBindingTriggerV1 { trigger }
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    assert!(matches!(
        tx.world
            .runtime_deferred_consents
            .get(&Key::Trigger(id.clone()))
            .unwrap()
            .consent,
        Consent::CurrentBindings
    ));
    let binding = log_binding(&tx);
    tx.world.runtime_selection.get_mut().restrictions = vec![RestrictionEntryV1 {
        definition: binding.interface,
        restriction: RestrictionV1::Disabled(1),
    }];
    assert!(
        trigger_ready(&*tx.world, &id, 1),
        "opt-in follows current bindings, ordinary dispatch still decides admission"
    );
    assert!(resolve_executing(&tx, &log()).is_err());
}

#[test]
fn multisig_approval_checks_the_original_capture_and_exact_body() {
    let state = state(true);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    register_multisig(&mut tx, &ALICE_ID, &[2; 32], &[log()], false).unwrap();
    let capture = check_multisig(&tx, &ALICE_ID, &[2; 32], &[log()])
        .unwrap()
        .unwrap();
    assert!(check_multisig(&tx, &ALICE_ID, &[2; 32], &[]).is_err());
    tx.deferred_expectations.push(Some(capture));
    assert!(check_call(&tx, &log()).is_ok());
    let binding = log_binding(&tx);
    tx.world.runtime_selection.get_mut().restrictions = vec![RestrictionEntryV1 {
        definition: binding.interface,
        restriction: RestrictionV1::AdmissionClosed(1),
    }];
    assert!(check_multisig(&tx, &ALICE_ID, &[2; 32], &[log()]).is_err());
    assert!(check_call(&tx, &log()).is_err());
    remove(&mut tx, multisig_key(&ALICE_ID, &[2; 32]));
    assert!(tx.world.runtime_deferred_consents.iter().next().is_none());
}

#[test]
fn retained_completion_cannot_reinterpret_captured_or_current_deferred_consent() {
    use iroha_data_model::{
        isi::runtime_obligation::RegisterRuntimeObligationV1,
        runtime::{definition::DefinitionDescriptorV1, lifecycle::ObligationKindV1},
    };
    let state = state(true);
    activate_additions_for_testing(
        &state.world,
        crate::runtime::catalog::additions::RUNTIME_LIFECYCLE.module,
    )
    .unwrap();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let captured = trigger("nested_capture");
    let current = trigger("nested_current");
    let captured_id = captured.id().clone();
    let current_id = current.id().clone();
    Register::trigger(captured)
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    RegisterCurrentBindingTriggerV1 { trigger: current }
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    register_multisig(&mut tx, &ALICE_ID, &[6; 32], &[log()], false).unwrap();
    register_multisig(&mut tx, &ALICE_ID, &[7; 32], &[log()], true).unwrap();
    let expectation = check_multisig(&tx, &ALICE_ID, &[6; 32], &[log()])
        .unwrap()
        .unwrap();
    crate::runtime::obligations::registry::register(
        &mut tx,
        &ALICE_ID,
        &RegisterRuntimeObligationV1 {
            contract: None,
            kind: ObligationKindV1::Escrow,
            terminal_effects: vec![log()],
            expires_ms: 61_000,
        },
    )
    .unwrap();
    let old = log_binding(&tx);
    let DefinitionDescriptorV1::Implementation(mut replacement) = tx
        .world
        .runtime_definitions
        .get(&old.implementation)
        .unwrap()
        .clone()
    else {
        panic!("compiled Log implementation");
    };
    // A distinct valid definition provides a changed binding while retaining the original
    // completion closure; this fixture exercises consent, without dispatching replacement code.
    replacement.limits.max_gas -= 1;
    let replacement = DefinitionDescriptorV1::Implementation(replacement);
    let new = replacement.id().unwrap();
    tx.world.runtime_definitions.insert(new, replacement);
    let canonical = tx.world.runtime_selection.get_mut();
    canonical.active.push(new);
    canonical.active.sort_unstable();
    canonical
        .bindings
        .iter_mut()
        .find(|binding| binding.wire_id == old.wire_id)
        .unwrap()
        .implementation = new;
    let current = check_multisig(&tx, &ALICE_ID, &[7; 32], &[log()]).unwrap();
    let refused = with_scope(&mut tx, current, |tx| {
        crate::runtime::obligations::registry::performing(tx, &ALICE_ID, &log(), |tx| {
            resolve_executing(tx, &log())
                .map(|_| ())
                .map_err(iroha_data_model::ValidationFail::InstructionFailed)
        })
    });
    assert!(
        refused.is_err(),
        "a per-instruction old completion cannot override a current-binding scope entered earlier"
    );
    assert!(tx.deferred_expectations.is_empty());
    assert!(tx.completing_obligation.is_none());
    assert!(
        tx.world
            .runtime_obligations
            .iter()
            .all(|(_, obligation)| obligation
                .terminal_effects
                .iter()
                .all(|effect| !effect.performed)),
        "caught refusal leaves every terminal effect pending"
    );
    crate::runtime::obligations::registry::performing(&mut tx, &ALICE_ID, &log(), |tx| {
        assert_eq!(log_binding(tx), old, "completion sees its retained binding");
        for proposal in [[6; 32], [7; 32]] {
            assert!(check_multisig(tx, &ALICE_ID, &proposal, &[log()]).is_err());
        }
        for id in [&captured_id, &current_id] {
            assert!(!trigger_ready_in_transaction(tx, id));
            let executable = tx
                .world
                .triggers
                .inspect_by_id(id, |action| action.executable().clone())
                .unwrap();
            assert!(trigger_expectation(tx, id, &ALICE_ID, &executable).is_err());
        }
        tx.deferred_expectations.push(Some(expectation.clone()));
        assert!(check_call(tx, &log()).is_err());
        tx.deferred_expectations.pop();
        Ok(())
    })
    .unwrap();
    assert!(
        check_multisig(&tx, &ALICE_ID, &[7; 32], &[log()])
            .unwrap()
            .is_none(),
        "ordinary opt-in keeps following the canonical bindings"
    );
    assert!(trigger_ready_in_transaction(&tx, &current_id));
    assert!(!trigger_ready_in_transaction(&tx, &captured_id));
}

#[test]
fn current_binding_deferred_scope_cannot_borrow_completion_restriction_exemptions() {
    use iroha_data_model::{
        isi::runtime_obligation::RegisterRuntimeObligationV1, runtime::lifecycle::ObligationKindV1,
    };
    let state = state(true);
    activate_additions_for_testing(
        &state.world,
        crate::runtime::catalog::additions::RUNTIME_LIFECYCLE.module,
    )
    .unwrap();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    register_multisig(&mut tx, &ALICE_ID, &[8; 32], &[log()], true).unwrap();
    crate::runtime::obligations::registry::register(
        &mut tx,
        &ALICE_ID,
        &RegisterRuntimeObligationV1 {
            contract: None,
            kind: ObligationKindV1::Escrow,
            terminal_effects: vec![log()],
            expires_ms: 61_000,
        },
    )
    .unwrap();
    // Model a root that retains this obligation while exposing exactly its captured
    // binding graph. Registry management calls cannot themselves be terminal effects.
    // This makes the completion and current scopes semantically identical, isolating
    // the ordinary restriction check from the separate changed-binding rejection.
    let captured_selection = tx
        .world
        .runtime_obligations
        .iter()
        .next()
        .unwrap()
        .1
        .capture
        .execution_selection(
            tx.world.runtime_selection.canonical(),
            &crate::runtime::change_set::PublishedDefinitions(&tx.world.runtime_definitions),
        )
        .unwrap();
    *tx.world.runtime_selection.get_mut() = captured_selection;
    let binding = log_binding(&tx);
    tx.world
        .runtime_selection
        .get_mut()
        .restrictions
        .push(RestrictionEntryV1 {
            definition: binding.interface,
            restriction: RestrictionV1::CompletionOnly(1),
        });
    crate::runtime::obligations::registry::performing(&mut tx, &ALICE_ID, &log(), |tx| {
        assert_eq!(
            tx.world.runtime_selection.get().bindings,
            tx.world.runtime_selection.canonical().bindings
        );
        assert!(
            resolve_executing(tx, &log()).is_ok(),
            "the direct committed terminal effect retains its capability"
        );
        let expectation = check_multisig(tx, &ALICE_ID, &[8; 32], &[log()]).unwrap();
        assert!(expectation.is_none());
        with_scope(tx, expectation, |tx| {
            assert!(
                resolve_executing(tx, &log()).is_err(),
                "nested deferred work cannot reuse the outer completion capability"
            );
        });
        assert!(tx.deferred_expectations.is_empty());
        assert!(resolve_executing(tx, &log()).is_ok());
        Ok(())
    })
    .unwrap();
}

#[test]
fn deferred_scope_restores_its_stack_on_caught_panic_and_leaves_inactive_roots_unchanged() {
    for selected in [false, true] {
        let state = state(selected);
        let mut block = state.block(header());
        let mut tx = block.transaction();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_scope(&mut tx, None, |tx| {
                assert_eq!(tx.deferred_expectations.len(), usize::from(selected));
                with_scope(tx, None, |tx| {
                    assert_eq!(tx.deferred_expectations.len(), 2 * usize::from(selected));
                    panic!("caught nested deferred failure");
                });
            });
        }));
        assert!(panic.is_err());
        assert!(tx.deferred_expectations.is_empty());
    }
}

#[test]
fn unrelated_same_field_storage_does_not_select_the_deferred_consent_service() {
    crate::runtime::catalog::test_catalog::with_additions(&[], || {
        let state = state(false);
        let mut block = state.block(header());
        let mut tx = block.transaction();
        Register::trigger(trigger("unconsented_baseline"))
            .execute(&ALICE_ID, &mut tx)
            .unwrap();
        let selection = tx.world.runtime_selection.canonical();
        let catalog = crate::runtime::release_catalog().unwrap();
        let unrelated_storage = catalog.world_storage["world.runtime_deferred_consents"];
        assert!(
            selection
                .installed_storage
                .binary_search(&unrelated_storage)
                .is_ok()
        );
        assert!(!enabled(selection).unwrap());
        assert_eq!(check_transition(&*tx.world, selection, selection), Ok(()));
    });
}
