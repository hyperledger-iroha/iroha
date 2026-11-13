//! Scoped native obligation admission, nested restoration and unwind cleanup.

use super::*;
use iroha_data_model::{
    block::BlockHeader,
    isi::Log,
    level::Level,
    runtime::{
        definition::{RestrictionEntryV1, RestrictionV1},
        obligation::CompletionCaptureV1,
    },
};
use mv::storage::StorageReadOnly as _;

fn state() -> crate::state::State {
    let state = crate::state::State::new_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    crate::runtime::activate_additions_for_testing(
        &state.world,
        crate::runtime::catalog::additions::RUNTIME_LIFECYCLE.module,
    )
    .unwrap();
    state
}

fn header() -> BlockHeader {
    BlockHeader::new(std::num::NonZeroU64::new(5).unwrap(), None, None, 1_000, 0)
}

fn log() -> InstructionBox {
    Log::new(Level::INFO, "native scope".to_owned()).into()
}

fn capture(transaction: &StateTransaction<'_, '_>) -> CompletionCaptureV1 {
    CompletionCaptureV1::capture(
        transaction.world.runtime_selection.canonical(),
        &PublishedDefinitions(&transaction.world.runtime_definitions),
        &[iroha_data_model::isi::instruction_wire_id(&log())
            .unwrap()
            .to_owned()],
        5,
    )
    .unwrap()
}

#[test]
fn native_progression_keeps_its_step_and_restores_nested_completion() {
    let state = state();
    let mut block = state.block(header());
    let mut transaction = block.transaction();
    let capture = capture(&transaction);
    let instruction = log();
    let wire = iroha_data_model::isi::instruction_wire_id(&instruction).unwrap();
    let definition = transaction
        .world
        .runtime_selection
        .canonical()
        .bindings
        .iter()
        .find(|binding| binding.wire_id == wire)
        .unwrap()
        .interface;
    transaction
        .world
        .runtime_selection
        .get_mut()
        .restrictions
        .push(RestrictionEntryV1 {
            definition,
            restriction: RestrictionV1::CompletionOnly(5),
        });
    let canonical = transaction.world.runtime_selection.canonical().clone();
    enter_native(
        &mut transaction,
        NativeExecution {
            capture: capture.clone(),
            step: Step::Continue,
        },
        |transaction| {
            assert_eq!(native_step(transaction, &log()), Some(Step::Continue));
            assert!(crate::runtime::resolve_executing(transaction, &log()).is_err());
            enter_native(
                transaction,
                NativeExecution {
                    capture,
                    step: Step::Complete,
                },
                |transaction| {
                    assert_eq!(native_step(transaction, &log()), Some(Step::Complete));
                    assert!(crate::runtime::resolve_executing(transaction, &log()).is_ok());
                    Ok(())
                },
            )?;
            assert_eq!(native_step(transaction, &log()), Some(Step::Continue));
            assert!(crate::runtime::resolve_executing(transaction, &log()).is_err());
            assert_eq!(transaction.world.runtime_selection.canonical(), &canonical);
            Ok(())
        },
    )
    .unwrap();
    assert!(transaction.native_execution.is_none());
    assert_eq!(transaction.world.runtime_selection.get(), &canonical);
}

#[test]
fn native_scope_releases_on_rejection_and_caught_unwind() {
    let state = state();
    let mut block = state.block(header());
    let mut transaction = block.transaction();
    let capture = capture(&transaction);
    let canonical = transaction.world.runtime_selection.canonical().clone();
    let result: Result<(), _> = enter_native(
        &mut transaction,
        NativeExecution {
            capture: capture.clone(),
            step: Step::Complete,
        },
        |_| Err(ValidationFail::NotPermitted("test rejection".into())),
    );
    assert!(result.is_err());
    assert!(transaction.native_execution.is_none());
    assert_eq!(transaction.world.runtime_selection.get(), &canonical);
    let result = iroha_panic_hook::catch_unwind_suppressed(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), ValidationFail> = enter_native(
            &mut transaction,
            NativeExecution {
                capture,
                step: Step::Complete,
            },
            |_| panic!("test native callback unwind"),
        );
    }));
    assert!(result.is_err());
    assert!(transaction.native_execution.is_none());
    assert_eq!(transaction.world.runtime_selection.get(), &canonical);
}

#[test]
fn deferred_body_cannot_borrow_native_scope_and_inactive_service_is_inert() {
    let state = state();
    let mut block = state.block(header());
    let mut transaction = block.transaction();
    with_native_obligation(&mut transaction, &log(), |transaction| {
        assert!(transaction.native_execution.is_none());
        Ok(())
    })
    .unwrap();
    let capture = capture(&transaction);
    enter_native(
        &mut transaction,
        NativeExecution {
            capture,
            step: Step::Complete,
        },
        |transaction| {
            assert_eq!(native_step(transaction, &log()), Some(Step::Complete));
            transaction.deferred_expectations.push(None);
            assert_eq!(native_step(transaction, &log()), None);
            transaction.deferred_expectations.pop();
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn registered_terminal_cannot_widen_native_progression_and_consumes_only_on_success() {
    use crate::runtime::obligations::registry;
    use iroha_data_model::{
        isi::runtime_obligation::RegisterRuntimeObligationV1,
        runtime::{lifecycle::ObligationKindV1, obligation::ObligationId},
    };
    use iroha_test_samples::ALICE_ID;

    let state = state();
    let mut block = state.block(header());
    let mut transaction = block.transaction();
    let terminal = log();
    registry::register(
        &mut transaction,
        &ALICE_ID,
        &RegisterRuntimeObligationV1 {
            contract: None,
            kind: ObligationKindV1::Escrow,
            terminal_effects: vec![terminal.clone()],
            expires_ms: 10_000,
        },
    )
    .unwrap();
    let capture = capture(&transaction);
    let id = ObligationId::derive(&ALICE_ID).unwrap();
    let before = transaction
        .world
        .runtime_obligations
        .get(&id)
        .unwrap()
        .clone();
    let definition = transaction
        .world
        .runtime_selection
        .canonical()
        .bindings
        .iter()
        .find(|binding| {
            binding.wire_id == iroha_data_model::isi::instruction_wire_id(&terminal).unwrap()
        })
        .unwrap()
        .interface;
    transaction
        .world
        .runtime_selection
        .get_mut()
        .restrictions
        .push(RestrictionEntryV1 {
            definition,
            restriction: RestrictionV1::CompletionOnly(5),
        });
    for (step, succeeds) in [(Step::Continue, false), (Step::Complete, true)] {
        let result = enter_native(
            &mut transaction,
            NativeExecution {
                capture: capture.clone(),
                step,
            },
            |transaction| {
                registry::performing(transaction, &ALICE_ID, &terminal, |transaction| {
                    crate::runtime::resolve_executing(transaction, &terminal)
                        .map(|_| ())
                        .map_err(ValidationFail::InstructionFailed)
                })
            },
        );
        assert_eq!(result.is_ok(), succeeds);
        if !succeeds {
            assert_eq!(
                transaction.world.runtime_obligations.get(&id),
                Some(&before)
            );
        }
        assert!(transaction.native_execution.is_none());
        assert!(transaction.completing_obligation.is_none());
    }
    assert!(
        transaction
            .world
            .runtime_obligations
            .get(&id)
            .unwrap()
            .terminal_effects
            .iter()
            .all(|effect| effect.performed)
    );
}

#[test]
fn conflicting_registered_or_nested_capture_cannot_replace_native_custody_context() {
    use crate::runtime::{catalog::additions::RESTRICTED_DATASPACE_READ, obligations::registry};
    use iroha_data_model::{
        isi::runtime_obligation::RegisterRuntimeObligationV1,
        runtime::{lifecycle::ObligationKindV1, obligation::ObligationId},
    };
    use iroha_test_samples::ALICE_ID;

    let state = state();
    let mut block = state.block(header());
    let mut transaction = block.transaction();
    let terminal = log();
    registry::register(
        &mut transaction,
        &ALICE_ID,
        &RegisterRuntimeObligationV1 {
            contract: None,
            kind: ObligationKindV1::Escrow,
            terminal_effects: vec![terminal.clone()],
            expires_ms: 10_000,
        },
    )
    .unwrap();
    let original = capture(&transaction);
    let before = transaction.world.runtime_selection.canonical().clone();
    let catalog = crate::runtime::release_catalog().unwrap();
    let addition = catalog
        .activation(
            RESTRICTED_DATASPACE_READ.module,
            &before,
            &PublishedDefinitions(&transaction.world.runtime_definitions),
            transaction.network_id,
            5,
            [0; 32],
        )
        .unwrap();
    let applied = addition
        .apply(
            &before,
            &PublishedDefinitions(&transaction.world.runtime_definitions),
        )
        .unwrap();
    for (id, descriptor) in applied.published {
        transaction.world.runtime_definitions.insert(id, descriptor);
    }
    *transaction.world.runtime_selection.get_mut() = applied.selection;
    let replacement = capture(&transaction);
    let id = ObligationId::derive(&ALICE_ID).unwrap();
    let record = transaction
        .world
        .runtime_obligations
        .get(&id)
        .unwrap()
        .clone();
    enter_native(
        &mut transaction,
        NativeExecution {
            capture: replacement,
            step: Step::Complete,
        },
        |transaction| {
            let executing = transaction.world.runtime_selection.get().clone();
            let result: Result<(), _> =
                registry::performing(transaction, &ALICE_ID, &terminal, |_| {
                    panic!("conflicting registered context must be refused before its callback")
                });
            assert!(result.is_err());
            let result: Result<(), _> = enter_native(
                transaction,
                NativeExecution {
                    capture: original,
                    step: Step::Complete,
                },
                |_| panic!("conflicting native context must be refused before its callback"),
            );
            assert!(result.is_err());
            assert_eq!(transaction.world.runtime_selection.get(), &executing);
            assert_eq!(
                transaction.world.runtime_obligations.get(&id),
                Some(&record)
            );
            Ok(())
        },
    )
    .unwrap();
    assert!(transaction.native_execution.is_none());
    assert!(transaction.completing_obligation.is_none());
}
