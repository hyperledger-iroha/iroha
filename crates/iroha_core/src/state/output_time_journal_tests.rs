//! Actual owned-overlay controls for bounded retry transition evidence.

use super::super::time_journal_test_support as support;
use super::*;
use iroha_data_model::{HasMetadata, events::time::Schedule};
use iroha_model_base::domain::DomainId;

fn value(used: u32, at: u64) -> Option<TimeTriggerRetryState> {
    Some(TimeTriggerRetryState {
        retries_used: used,
        next_retry_at_ms: at,
    })
}

#[test]
fn time_retry_journal_preserves_originals_and_nested_encounter_order_until_drop() {
    let (state, id) = support::state_fixture(None, true);
    let mut block = state.block(support::header());
    let original = support::original_action(&block, &id);
    let pool = AllocationBudget::new(1_000_000);
    let other_pool = AllocationBudget::new(1_000_000);
    let journal = ApplyingTimeRetryJournal::new(
        OutputTransaction::new(&mut block).unwrap(),
        &pool,
        4,
        2,
        10_000_000,
        support::limits(),
    )
    .unwrap();
    let evidence = journal
        .set_retry_state(&id, value(1, 10))
        .unwrap()
        .with_scope(|scope| {
            scope.set_retry_state(&id, value(2, 20))?;
            scope.with_scope(|inner| inner.set_retry_state(&id, None))
        })
        .unwrap()
        .set_retry_state(&id, value(3, 30))
        .unwrap()
        .finish()
        .unwrap();
    assert!(evidence.belongs_to(&pool));
    assert!(!evidence.belongs_to(&other_pool));
    let records = evidence.records();
    assert_eq!(records.len(), 4);
    assert_eq!(
        records
            .iter()
            .map(super::OwnedTimeRetryTransition::depth)
            .collect::<Vec<_>>(),
        [0, 1, 2, 0]
    );
    for (index, row) in records.iter().enumerate() {
        assert_eq!(row.ordinal(), index as u64);
        assert_eq!(row.before().unwrap().id_bytes(), b"journal_retry");
        assert_eq!(row.after().unwrap().id_bytes(), b"journal_retry");
        assert_eq!(row.before().unwrap().action_bytes(), original);
        assert_eq!(row.after().unwrap().action_bytes(), original);
        assert!(row.before().unwrap().belongs_to(&pool));
        assert!(row.after().unwrap().belongs_to(&pool));
    }
    assert_eq!(records[0].before().unwrap().retry_state(), None);
    assert_eq!(records[0].after().unwrap().retry_state(), Some((1, 10)));
    assert_eq!(records[1].before().unwrap().retry_state(), Some((1, 10)));
    assert_eq!(records[1].after().unwrap().retry_state(), Some((2, 20)));
    assert_eq!(records[2].before().unwrap().retry_state(), Some((2, 20)));
    assert_eq!(records[2].after().unwrap().retry_state(), None);
    assert_eq!(records[3].before().unwrap().retry_state(), None);
    assert_eq!(records[3].after().unwrap().retry_state(), Some((3, 30)));
    let source_bytes = original.len() + b"journal_retry".len();
    assert_eq!(
        pool.reserved_bytes(),
        4 * std::mem::size_of::<OwnedTimeRetryTransition>() + 8 * source_bytes
    );
    // The staged evidence still owns the actual disposable State/witness overlay.
    drop(evidence);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::retry(&block, &id), None);
    assert_eq!(support::original_action(&block, &id), original);
}

#[test]
fn time_retry_journal_capacity_refusal_drops_earlier_real_mutation() {
    let (state, id) = support::state_fixture(None, true);
    let mut block = state.block(support::header());
    let pool = AllocationBudget::new(1_000_000);
    let journal = ApplyingTimeRetryJournal::new(
        OutputTransaction::new(&mut block).unwrap(),
        &pool,
        1,
        2,
        10_000_000,
        support::limits(),
    )
    .unwrap()
    .set_retry_state(&id, value(1, 10))
    .unwrap();
    assert!(matches!(
        journal.set_retry_state(&id, value(2, 20)),
        Err(TimeRetryJournalError::RecordCapacity)
    ));
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::retry(&block, &id), None);
}

#[test]
fn time_retry_journal_after_capture_allocation_refusal_rolls_back_the_actual_write() {
    let (state, id) = support::state_fixture(None, true);
    let mut block = state.block(support::header());
    let original = support::original_action(&block, &id);
    // Exactly one row plus its first actual ID/Action capture; no after-ID capacity.
    let bytes =
        std::mem::size_of::<OwnedTimeRetryTransition>() + original.len() + b"journal_retry".len();
    let pool = AllocationBudget::new(bytes);
    let journal = ApplyingTimeRetryJournal::new(
        OutputTransaction::new(&mut block).unwrap(),
        &pool,
        1,
        2,
        10_000_000,
        support::limits(),
    )
    .unwrap();
    assert!(matches!(
        journal.set_retry_state(&id, value(1, 10)),
        Err(TimeRetryJournalError::Source(
            ApplyingTimeSourceError::IdBacking(_)
        ))
    ));
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::retry(&block, &id), None);
    assert_eq!(support::original_action(&block, &id), original);
}

#[test]
fn time_retry_journal_original_source_work_refusal_is_not_missing_action() {
    for nested in [false, true] {
        let (state, id) = support::state_fixture(None, true);
        let mut block = state.block(support::header());
        let pool = AllocationBudget::new(1_000_000);
        let journal = ApplyingTimeRetryJournal::new(
            OutputTransaction::new(&mut block).unwrap(),
            &pool,
            1,
            2,
            0,
            support::limits(),
        )
        .unwrap();
        let result = if nested {
            journal.with_scope(|scope| {
                assert!(scope.set_retry_state(&id, value(1, 10)).is_err());
                Ok(()) // The original Source error survives swallowing its marker.
            })
        } else {
            journal.set_retry_state(&id, value(1, 10))
        };
        assert!(matches!(
            result,
            Err(TimeRetryJournalError::Source(
                ApplyingTimeSourceError::Work(_)
            ))
        ));
        drop(result);
        assert_eq!(pool.reserved_bytes(), 0);
        assert_eq!(support::retry(&block, &id), None);
    }
}

#[test]
fn time_retry_journal_missing_typed_action_consumes_the_overlay() {
    let (state, id) = support::state_fixture(None, true);
    let mut block = state.block(support::header());
    let pool = AllocationBudget::new(1_000_000);
    let journal = ApplyingTimeRetryJournal::new(
        OutputTransaction::new(&mut block).unwrap(),
        &pool,
        2,
        2,
        10_000_000,
        support::limits(),
    )
    .unwrap()
    .set_retry_state(&id, value(1, 10))
    .unwrap();
    assert!(matches!(
        journal.set_retry_state(&"missing".parse().unwrap(), None),
        Err(TimeRetryJournalError::MissingTimeSource)
    ));
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::retry(&block, &id), None);
}

#[test]
fn time_retry_journal_depth_refusal_rolls_back_and_never_enters_refused_body() {
    let (state, id) = support::state_fixture(None, true);
    let mut block = state.block(support::header());
    let pool = AllocationBudget::new(1_000_000);
    let journal = ApplyingTimeRetryJournal::new(
        OutputTransaction::new(&mut block).unwrap(),
        &pool,
        2,
        0,
        10_000_000,
        support::limits(),
    )
    .unwrap()
    .set_retry_state(&id, value(1, 10))
    .unwrap();
    let mut entered = false;
    assert!(matches!(
        journal.with_scope(|_| {
            entered = true;
            Ok(())
        }),
        Err(TimeRetryJournalError::DepthLimit)
    ));
    assert!(!entered);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::retry(&block, &id), None);
}

#[test]
fn time_retry_journal_ignored_scope_refusal_keeps_original_error_and_rolls_back() {
    let (state, id) = support::state_fixture(None, true);
    let mut block = state.block(support::header());
    let pool = AllocationBudget::new(1_000_000);
    let journal = ApplyingTimeRetryJournal::new(
        OutputTransaction::new(&mut block).unwrap(),
        &pool,
        1,
        2,
        10_000_000,
        support::limits(),
    )
    .unwrap()
    .set_retry_state(&id, value(1, 10))
    .unwrap();
    let result = journal.with_scope(|scope| {
        assert!(scope.set_retry_state(&id, value(2, 20)).is_err());
        // This deliberately swallows the marker; the original capacity error stays owned.
        Ok(())
    });
    assert!(matches!(result, Err(TimeRetryJournalError::RecordCapacity)));
    drop(result);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::retry(&block, &id), None);
}

#[test]
fn time_retry_journal_caught_nested_unwind_still_requires_overlay_rollback() {
    let (state, id) = support::state_fixture(None, true);
    let mut block = state.block(support::header());
    let pool = AllocationBudget::new(1_000_000);
    let journal = ApplyingTimeRetryJournal::new(
        OutputTransaction::new(&mut block).unwrap(),
        &pool,
        2,
        2,
        10_000_000,
        support::limits(),
    )
    .unwrap();
    let result = journal.with_scope(|scope| {
        scope.set_retry_state(&id, value(1, 10))?;
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = scope.with_scope(|inner| {
                inner.set_retry_state(&id, value(2, 20))?;
                panic!("caught nested interruption")
            });
        }));
        assert!(interrupted.is_err());
        Ok(())
    });
    assert!(matches!(result, Err(TimeRetryJournalError::ScopeAborted)));
    drop(result);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::retry(&block, &id), None);
}

#[test]
fn time_retry_journal_unwind_drops_actual_overlay_and_all_charged_backing() {
    let (state, id) = support::state_fixture(None, true);
    let mut block = state.block(support::header());
    let pool = AllocationBudget::new(1_000_000);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let journal = ApplyingTimeRetryJournal::new(
            OutputTransaction::new(&mut block).unwrap(),
            &pool,
            1,
            2,
            10_000_000,
            support::limits(),
        )
        .unwrap()
        .set_retry_state(&id, value(1, 10))
        .unwrap();
        let _ = journal.with_scope(|_| panic!("test interruption while owner is live"));
    }));
    assert!(result.is_err());
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::retry(&block, &id), None);
}

#[test]
fn time_retry_journal_constructor_backing_refusal_drops_its_original_overlay() {
    let (state, id) = support::state_fixture(None, true);
    let mut block = state.block(support::header());
    let pool = AllocationBudget::new(0);
    let mut output = OutputTransaction::new(&mut block).unwrap();
    assert!(
        output
            .transaction
            .as_mut()
            .unwrap()
            .world
            .triggers
            .set_time_trigger_retry_state(&id, value(1, 10))
    );
    assert!(matches!(
        ApplyingTimeRetryJournal::new(output, &pool, 1, 2, 10_000_000, support::limits()),
        Err(TimeRetryJournalError::RecordBacking(_))
    ));
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::retry(&block, &id), None);
}

fn install_time_instruction_permission(
    block: &mut crate::state::StateBlock<'_>,
    id: &TriggerId,
) -> iroha_data_model::permission::Permission {
    let permission: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::trigger::CanExecuteTrigger {
            trigger: id.clone(),
        }
        .into();
    let mut setup = block.transaction();
    assert!(
        setup
            .world
            .add_account_permission(&iroha_test_samples::ALICE_ID, permission.clone())
    );
    setup.apply();
    permission
}

#[test]
fn time_instruction_journal_shares_retry_order_and_retains_actual_mint_burn_remove_images() {
    use iroha_test_samples::ALICE_ID;
    let (state, id) = support::state_fixture_with_repetitions(None, true, 2, true);
    let mut block = state.block(support::header());
    let original = support::original_action(&block, &id);
    let pool = AllocationBudget::new(1_000_000);
    let evidence = ApplyingTimeRetryJournal::new(
        OutputTransaction::new(&mut block).unwrap(),
        &pool,
        4,
        2,
        10_000_000,
        support::limits(),
    )
    .unwrap()
    .set_retry_state(&id, value(1, 20))
    .unwrap()
    .with_scope(|scope| {
        scope.mint_time_repetitions(&ALICE_ID, &id, 3)?;
        scope.with_scope(|inner| inner.burn_time_repetitions(&ALICE_ID, &id, 1))
    })
    .unwrap()
    .unregister_time(&ALICE_ID, &id)
    .unwrap()
    .finish()
    .unwrap();
    let records = evidence.records();
    assert_eq!(records.len(), 4);
    assert_eq!(
        records
            .iter()
            .map(super::OwnedTimeRetryTransition::ordinal)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert_eq!(
        records
            .iter()
            .map(super::OwnedTimeRetryTransition::depth)
            .collect::<Vec<_>>(),
        [0, 1, 2, 0]
    );
    assert_eq!(
        records
            .iter()
            .map(super::OwnedTimeRetryTransition::kind)
            .collect::<Vec<_>>(),
        [
            TimeJournalMutation::RetryState,
            TimeJournalMutation::MintRepetitions(3),
            TimeJournalMutation::BurnRepetitions(1),
            TimeJournalMutation::Unregister,
        ]
    );
    assert_eq!(records[0].before().unwrap().action_bytes(), original);
    assert_eq!(records[0].after().unwrap().action_bytes(), original);
    assert_ne!(
        records[1].before().unwrap().action_bytes(),
        records[1].after().unwrap().action_bytes()
    );
    assert_ne!(
        records[2].before().unwrap().action_bytes(),
        records[2].after().unwrap().action_bytes()
    );
    for pair in records.windows(2) {
        assert_eq!(
            pair[0].after().unwrap().action_bytes(),
            pair[1].before().unwrap().action_bytes()
        );
        assert_eq!(
            pair[0].after().unwrap().retry_state(),
            pair[1].before().unwrap().retry_state()
        );
    }
    for row in records {
        assert_eq!(row.before().unwrap().id_bytes(), b"journal_retry");
        assert!(row.before().unwrap().belongs_to(&pool));
        if let Some(after) = row.after() {
            assert_eq!(after.id_bytes(), b"journal_retry");
            assert_eq!(after.retry_state(), Some((1, 20)));
            assert!(after.belongs_to(&pool));
        }
    }
    assert!(records[3].after().is_none());
    assert!(evidence.belongs_to(&pool));
    drop(evidence);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::original_action(&block, &id), original);
    assert_eq!(support::retry(&block, &id), None);
}

#[test]
fn time_instruction_journal_burn_zero_cleans_real_permission_and_drop_restores_it() {
    use crate::smartcontracts::isi::triggers::set::SetReadOnly;
    use iroha_test_samples::ALICE_ID;
    use mv::storage::StorageReadOnly;
    let (state, id) = support::state_fixture(Some((1, 20)), true);
    let mut block = state.block(support::header());
    let permission = install_time_instruction_permission(&mut block, &id);
    let original = support::original_action(&block, &id);
    let pool = AllocationBudget::new(1_000_000);
    let journal = ApplyingTimeRetryJournal::new(
        OutputTransaction::new(&mut block).unwrap(),
        &pool,
        1,
        1,
        10_000_000,
        support::limits(),
    )
    .unwrap()
    .burn_time_repetitions(&ALICE_ID, &id, 2)
    .unwrap();
    let transaction = journal.output.transaction.as_ref().unwrap();
    assert!(
        transaction
            .world
            .triggers
            .time_triggers()
            .get(&id)
            .is_none()
    );
    assert!(
        !transaction
            .world
            .account_permissions
            .get(&ALICE_ID)
            .is_some_and(|permissions| permissions.contains(&permission))
    );
    let evidence = journal.finish().unwrap();
    assert_eq!(
        evidence.records()[0].kind(),
        TimeJournalMutation::BurnRepetitions(2)
    );
    assert_eq!(
        evidence.records()[0].before().unwrap().action_bytes(),
        original
    );
    assert_eq!(
        evidence.records()[0].before().unwrap().retry_state(),
        Some((1, 20))
    );
    assert!(evidence.records()[0].after().is_none());
    drop(evidence);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::original_action(&block, &id), original);
    assert_eq!(support::retry(&block, &id), Some((1, 20)));
    assert!(
        block
            .world
            .account_permissions
            .get(&ALICE_ID)
            .unwrap()
            .contains(&permission)
    );
}

#[test]
fn time_instruction_journal_unregister_cleanup_rolls_back_after_swallowed_scope_refusal() {
    use iroha_test_samples::ALICE_ID;
    use mv::storage::StorageReadOnly;
    let (state, id) = support::state_fixture(None, true);
    let mut block = state.block(support::header());
    let permission = install_time_instruction_permission(&mut block, &id);
    let original = support::original_action(&block, &id);
    let pool = AllocationBudget::new(1_000_000);
    let result = ApplyingTimeRetryJournal::new(
        OutputTransaction::new(&mut block).unwrap(),
        &pool,
        1,
        1,
        10_000_000,
        support::limits(),
    )
    .unwrap()
    .with_scope(|scope| {
        scope.unregister_time(&ALICE_ID, &id)?;
        let transaction = scope.journal.output.transaction.as_ref().unwrap();
        assert!(
            !transaction
                .world
                .account_permissions
                .get(&ALICE_ID)
                .is_some_and(|permissions| permissions.contains(&permission))
        );
        assert!(scope.burn_time_repetitions(&ALICE_ID, &id, 1).is_err());
        Ok(()) // Original row-capacity refusal remains sticky after real cleanup.
    });
    assert!(matches!(result, Err(TimeRetryJournalError::RecordCapacity)));
    drop(result);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::original_action(&block, &id), original);
    assert!(
        block
            .world
            .account_permissions
            .get(&ALICE_ID)
            .unwrap()
            .contains(&permission)
    );
}

#[test]
fn time_instruction_journal_preserves_original_overflow_underflow_and_mintable_errors() {
    use iroha_data_model::isi::error::MathError;
    use iroha_test_samples::ALICE_ID;
    for case in 0..3 {
        let (state, id) = support::state_fixture_with_repetitions(
            None,
            true,
            if case == 0 { u32::MAX } else { 2 },
            case != 2,
        );
        let mut block = state.block(support::header());
        let original = support::original_action(&block, &id);
        let pool = AllocationBudget::new(1_000_000);
        let journal = ApplyingTimeRetryJournal::new(
            OutputTransaction::new(&mut block).unwrap(),
            &pool,
            2,
            1,
            10_000_000,
            support::limits(),
        )
        .unwrap()
        .set_retry_state(&id, value(1, 20))
        .unwrap();
        let result = if case == 1 {
            journal.burn_time_repetitions(&ALICE_ID, &id, 3)
        } else {
            journal.mint_time_repetitions(&ALICE_ID, &id, 1)
        };
        assert!(matches!(
            result,
            Err(TimeRetryJournalError::Execution(
                InstructionExecutionError::Math(MathError::Overflow)
            ))
        ));
        drop(result);
        assert_eq!(pool.reserved_bytes(), 0);
        assert_eq!(support::original_action(&block, &id), original);
        assert_eq!(support::retry(&block, &id), None);
    }
}

#[test]
fn time_instruction_journal_after_capture_refusal_restores_real_burn_mutation() {
    use iroha_test_samples::ALICE_ID;
    let (state, id) = support::state_fixture(Some((1, 20)), true);
    let mut block = state.block(support::header());
    let original = support::original_action(&block, &id);
    let pool = AllocationBudget::new(
        std::mem::size_of::<OwnedTimeRetryTransition>() + original.len() + b"journal_retry".len(),
    );
    let result = ApplyingTimeRetryJournal::new(
        OutputTransaction::new(&mut block).unwrap(),
        &pool,
        1,
        1,
        10_000_000,
        support::limits(),
    )
    .unwrap()
    .burn_time_repetitions(&ALICE_ID, &id, 1);
    assert!(matches!(
        result,
        Err(TimeRetryJournalError::Source(
            ApplyingTimeSourceError::IdBacking(_)
        ))
    ));
    drop(result);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::original_action(&block, &id), original);
    assert_eq!(support::retry(&block, &id), Some((1, 20)));
}

#[test]
fn time_instruction_journal_unwind_after_implicit_removal_restores_permission_and_original() {
    use iroha_test_samples::ALICE_ID;
    use mv::storage::StorageReadOnly;
    let (state, id) = support::state_fixture(Some((1, 20)), true);
    let mut block = state.block(support::header());
    let permission = install_time_instruction_permission(&mut block, &id);
    let original = support::original_action(&block, &id);
    let pool = AllocationBudget::new(1_000_000);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let journal = ApplyingTimeRetryJournal::new(
            OutputTransaction::new(&mut block).unwrap(),
            &pool,
            1,
            1,
            10_000_000,
            support::limits(),
        )
        .unwrap();
        let _ = journal.with_scope(|scope| {
            scope.burn_time_repetitions(&ALICE_ID, &id, 2)?;
            panic!("unwind after genuine implicit removal");
        });
    }));
    assert!(result.is_err());
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::original_action(&block, &id), original);
    assert_eq!(support::retry(&block, &id), Some((1, 20)));
    assert!(
        block
            .world
            .account_permissions
            .get(&ALICE_ID)
            .unwrap()
            .contains(&permission)
    );
}

// Register controls call the actual entrypoint inside the same disposable owner.
// Setup account instructions are component preparation, not executor admission.
fn register_journal_accounts(block: &mut crate::state::StateBlock<'_>) {
    use iroha_data_model::prelude::*;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    let mut setup = block.transaction();
    Register::domain(Domain::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
    ))
    .execute(&ALICE_ID, &mut setup)
    .unwrap();
    Register::account(Account::new(ALICE_ID.clone()))
        .execute(&ALICE_ID, &mut setup)
        .unwrap();
    Register::account(Account::new(BOB_ID.clone()))
        .execute(&ALICE_ID, &mut setup)
        .unwrap();
    setup.apply();
}

fn register_journal_time_instruction(
    id: &TriggerId,
    repetitions: u32,
    period: Option<std::time::Duration>,
    arguments: Option<Vec<u8>>,
) -> Register<Trigger> {
    use iroha_data_model::{
        prelude::*,
        smart_contract::ContractAddress,
        transaction::executable::{
            ContractArgumentRecord, ContractInvocation, ExecutableBatchItem,
        },
    };
    use iroha_test_samples::ALICE_ID;
    let executable = arguments.map_or_else(
        || Executable::Instructions(Vec::<InstructionBox>::new().into()),
        |arguments| Executable::Batch(vec![ExecutableBatchItem::ContractCall(ContractInvocation {
            contract_address: ContractAddress::derive(
                &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E".parse().unwrap(),
                &ALICE_ID,
                7,
                iroha_model_base::topology::DataSpaceId::UNIVERSAL,
            ).unwrap(),
            expected_code_hash: iroha_crypto::Hash::new(b"applying Time registration fixture"),
            entrypoint: "execute_scheduled_payment".to_owned(),
            arguments: Some(ContractArgumentRecord::try_new(arguments).unwrap()),
        })].into()),
    );
    let schedule = Schedule::starting_at(std::time::Duration::from_millis(10_000));
    let schedule = if let Some(period) = period {
        schedule.with_period(period)
    } else {
        schedule
    };
    let mut action = Action::new(
        executable,
        Repeats::Exactly(repetitions),
        ALICE_ID.clone(),
        TimeEventFilter(ExecutionTime::Schedule(schedule)),
    )
    .unwrap();
    action.retry_policy = Some(TimeTriggerRetryPolicy {
        max_retries: std::num::NonZeroU32::new(3).unwrap(),
        retry_after_ms: std::num::NonZeroU64::new(5_000).unwrap(),
    });
    Register::trigger(Trigger::new(id.clone(), action))
}

fn register_journal_by_call_instruction(id: &TriggerId) -> Register<Trigger> {
    use iroha_data_model::prelude::*;
    Register::trigger(Trigger::new(
        id.clone(),
        Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Indefinitely,
            iroha_test_samples::ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new(),
        )
        .unwrap(),
    ))
}

#[test]
fn time_register_journal_retains_actual_metadata_deferred_bytes_and_shared_retry_order() {
    use crate::smartcontracts::isi::triggers::set::SetReadOnly;
    use iroha_test_samples::ALICE_ID;
    use mv::storage::StorageReadOnly;
    for arguments in [None, Some(vec![0xA5; 64])] {
        let (state, old_id) = support::state_fixture(None, true);
        let mut block = state.block(support::header());
        register_journal_accounts(&mut block);
        let old_action = support::original_action(&block, &old_id);
        let id: TriggerId = "journal_registered".parse().unwrap();
        let instruction = register_journal_time_instruction(&id, 1, None, arguments);
        let input_action = norito::encode_canonical(instruction.object().action()).unwrap();
        let input_executable = instruction.object().action().executable().clone();
        let pool = AllocationBudget::new(1_000_000);
        let journal = ApplyingTimeRetryJournal::new(
            OutputTransaction::new(&mut block).unwrap(),
            &pool,
            3,
            1,
            10_000_000,
            support::limits(),
        )
        .unwrap()
        .set_retry_state(&old_id, value(1, 20))
        .unwrap()
        .with_scope(|scope| scope.register_time(&ALICE_ID, instruction))
        .unwrap()
        .set_retry_state(&id, value(1, 52))
        .unwrap();
        let transaction = journal.output.transaction.as_ref().unwrap();
        let registered = transaction
            .world
            .triggers
            .triggers_iter()
            .find(|row| row.id() == &id)
            .unwrap();
        assert_eq!(registered.action().executable(), &input_executable);
        assert_eq!(
            registered
                .action()
                .metadata()
                .get::<str>("__registered_block_height")
                .unwrap()
                .get(),
            "2"
        );
        assert_eq!(
            registered
                .action()
                .metadata()
                .get::<str>("__registered_at_ms")
                .unwrap()
                .get(),
            "42"
        );
        let actual_action = norito::encode_canonical(registered.action()).unwrap();
        assert_ne!(actual_action, input_action);
        assert!(
            transaction
                .world
                .triggers
                .time_triggers()
                .get(&id)
                .is_some()
        );
        let evidence = journal.finish().unwrap();
        let rows = evidence.records();
        assert_eq!(
            rows.iter()
                .map(super::OwnedTimeRetryTransition::kind)
                .collect::<Vec<_>>(),
            [
                TimeJournalMutation::RetryState,
                TimeJournalMutation::Register,
                TimeJournalMutation::RetryState,
            ]
        );
        assert_eq!(
            rows.iter()
                .map(super::OwnedTimeRetryTransition::ordinal)
                .collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(
            rows.iter()
                .map(super::OwnedTimeRetryTransition::depth)
                .collect::<Vec<_>>(),
            [0, 1, 0]
        );
        assert!(rows[1].before().is_none());
        assert_eq!(rows[1].after().unwrap().id_bytes(), b"journal_registered");
        assert_eq!(rows[1].after().unwrap().action_bytes(), actual_action);
        assert_eq!(rows[1].after().unwrap().retry_state(), None);
        assert_eq!(
            rows[2].before().unwrap().action_bytes(),
            rows[1].after().unwrap().action_bytes()
        );
        assert_eq!(rows[2].before().unwrap().retry_state(), None);
        assert_eq!(rows[2].after().unwrap().retry_state(), Some((1, 52)));
        assert!(evidence.belongs_to(&pool));
        // The malformed inner bytes were deliberately accepted by real deferred
        // Register policy. Capturing them does not run the canonical/schema scanner.
        drop(evidence);
        assert_eq!(pool.reserved_bytes(), 0);
        assert!(
            block
                .world
                .triggers
                .triggers_iter()
                .all(|row| row.id() != &id)
        );
        assert_eq!(support::original_action(&block, &old_id), old_action);
        assert_eq!(support::retry(&block, &old_id), None);
    }
}

#[test]
fn time_register_journal_remove_readd_preserves_incarnation_sources_cleanup_and_rollback() {
    use crate::smartcontracts::isi::triggers::set::SetReadOnly;
    use iroha_test_samples::ALICE_ID;
    use mv::storage::StorageReadOnly;
    let (state, id) = support::state_fixture(Some((1, 20)), true);
    let mut block = state.block(support::header());
    register_journal_accounts(&mut block);
    let permission = install_time_instruction_permission(&mut block, &id);
    let original = support::original_action(&block, &id);
    let pool = AllocationBudget::new(1_000_000);
    let instruction = register_journal_time_instruction(&id, 1, None, None);
    let journal = ApplyingTimeRetryJournal::new(
        OutputTransaction::new(&mut block).unwrap(),
        &pool,
        3,
        1,
        10_000_000,
        support::limits(),
    )
    .unwrap()
    .unregister_time(&ALICE_ID, &id)
    .unwrap()
    .register_time(&ALICE_ID, instruction)
    .unwrap()
    .set_retry_state(&id, value(1, 60))
    .unwrap();
    let transaction = journal.output.transaction.as_ref().unwrap();
    assert!(
        transaction
            .world
            .triggers
            .time_triggers()
            .get(&id)
            .is_some()
    );
    assert!(
        !transaction
            .world
            .account_permissions
            .get(&ALICE_ID)
            .is_some_and(|permissions| permissions.contains(&permission))
    );
    let actual = norito::encode_canonical(
        transaction
            .world
            .triggers
            .triggers_iter()
            .find(|row| row.id() == &id)
            .unwrap()
            .action(),
    )
    .unwrap();
    let evidence = journal.finish().unwrap();
    let rows = evidence.records();
    assert_eq!(
        rows.iter()
            .map(super::OwnedTimeRetryTransition::ordinal)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(
        rows.iter()
            .map(super::OwnedTimeRetryTransition::kind)
            .collect::<Vec<_>>(),
        [
            TimeJournalMutation::Unregister,
            TimeJournalMutation::Register,
            TimeJournalMutation::RetryState,
        ]
    );
    assert_eq!(rows[0].before().unwrap().action_bytes(), original);
    assert_eq!(rows[0].before().unwrap().retry_state(), Some((1, 20)));
    assert!(rows[0].after().is_none());
    assert!(rows[1].before().is_none());
    assert_eq!(rows[1].after().unwrap().action_bytes(), actual);
    assert_eq!(rows[1].after().unwrap().retry_state(), None);
    assert_ne!(rows[1].after().unwrap().action_bytes(), original);
    assert_eq!(rows[2].before().unwrap().retry_state(), None);
    assert_eq!(rows[2].after().unwrap().retry_state(), Some((1, 60)));
    drop(evidence);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::original_action(&block, &id), original);
    assert_eq!(support::retry(&block, &id), Some((1, 20)));
    assert!(
        block
            .world
            .account_permissions
            .get(&ALICE_ID)
            .unwrap()
            .contains(&permission)
    );
}

#[test]
fn time_register_journal_preserves_time_and_other_table_duplicate_error_after_prior_mutation() {
    use crate::smartcontracts::isi::triggers::set::SetReadOnly;
    use iroha_data_model::isi::{InstructionType, error::RepetitionError};
    use iroha_test_samples::ALICE_ID;
    use mv::storage::StorageReadOnly;
    for other_table in [false, true] {
        let (state, old_id) = support::state_fixture(None, true);
        let mut block = state.block(support::header());
        register_journal_accounts(&mut block);
        let id = if other_table {
            "journal_by_call_duplicate".parse().unwrap()
        } else {
            old_id.clone()
        };
        if other_table {
            let mut setup = block.transaction();
            register_journal_by_call_instruction(&id)
                .execute(&ALICE_ID, &mut setup)
                .unwrap();
            setup.apply();
        }
        let original = support::original_action(&block, &old_id);
        let duplicate_original = norito::encode_canonical(
            block
                .world
                .triggers
                .triggers_iter()
                .find(|row| row.id() == &id)
                .unwrap()
                .action(),
        )
        .unwrap();
        let pool = AllocationBudget::new(1_000_000);
        let journal = ApplyingTimeRetryJournal::new(
            OutputTransaction::new(&mut block).unwrap(),
            &pool,
            2,
            1,
            10_000_000,
            support::limits(),
        )
        .unwrap()
        .set_retry_state(&old_id, value(1, 20))
        .unwrap();
        if other_table {
            // Real global duplicate, despite genuine typed-Time absence.
            assert!(
                journal
                    .output
                    .transaction
                    .as_ref()
                    .unwrap()
                    .world
                    .triggers
                    .time_triggers()
                    .get(&id)
                    .is_none()
            );
        }
        match journal.with_scope(|scope| {
            assert!(
                scope
                    .register_time(
                        &ALICE_ID,
                        register_journal_time_instruction(&id, 1, None, None)
                    )
                    .is_err()
            );
            Ok(()) // Swallowing its marker cannot erase the original Repetition.
        }) {
            Err(TimeRetryJournalError::Execution(InstructionExecutionError::Repetition(
                original,
            ))) => {
                assert_eq!(
                    original,
                    RepetitionError {
                        instruction: InstructionType::Register,
                        id: id.clone().into()
                    }
                );
            }
            Err(other) => panic!("original Register duplicate error changed: {other:?}"),
            Ok(_) => panic!("global duplicate was accepted"),
        }
        assert_eq!(pool.reserved_bytes(), 0);
        assert_eq!(support::original_action(&block, &old_id), original);
        assert_eq!(support::retry(&block, &old_id), None);
        assert_eq!(
            norito::encode_canonical(
                block
                    .world
                    .triggers
                    .triggers_iter()
                    .find(|row| row.id() == &id)
                    .unwrap()
                    .action()
            )
            .unwrap(),
            duplicate_original
        );
    }
}

#[test]
fn time_register_journal_preserves_actual_authority_period_and_mintable_refusals() {
    use crate::smartcontracts::isi::triggers::set::SetReadOnly;
    use iroha_data_model::isi::error::{InvalidParameterError, MathError};
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    for case in 0..3 {
        let (state, old_id) = support::state_fixture(None, true);
        let mut block = state.block(support::header());
        register_journal_accounts(&mut block);
        let id: TriggerId = "journal_register_refused".parse().unwrap();
        let instruction = register_journal_time_instruction(
            &id,
            if case == 2 { 2 } else { 1 },
            (case == 1).then_some(std::time::Duration::ZERO),
            None,
        );
        let pool = AllocationBudget::new(1_000_000);
        let journal = ApplyingTimeRetryJournal::new(
            OutputTransaction::new(&mut block).unwrap(),
            &pool,
            2,
            1,
            10_000_000,
            support::limits(),
        )
        .unwrap()
        .set_retry_state(&old_id, value(1, 20))
        .unwrap();
        let authority = if case == 0 { &*BOB_ID } else { &*ALICE_ID };
        let original = match journal.register_time(authority, instruction) {
            Err(TimeRetryJournalError::Execution(original)) => original,
            Err(other) => panic!("expected original Register refusal: {other:?}"),
            Ok(_) => panic!("invalid registration succeeded"),
        };
        match (case, original) {
            (
                0,
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    message,
                )),
            ) => assert!(message.contains("CanRegisterTrigger")),
            (
                1,
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    message,
                )),
            ) => assert!(message.contains("period must be greater than zero")),
            (2, InstructionExecutionError::Math(MathError::Overflow)) => {}
            (_, other) => panic!("Register changed the original error variant: {other:?}"),
        }
        assert_eq!(pool.reserved_bytes(), 0);
        assert_eq!(support::retry(&block, &old_id), None);
        assert!(
            block
                .world
                .triggers
                .triggers_iter()
                .all(|row| row.id() != &id)
        );
    }
}

#[test]
fn time_register_journal_after_capture_and_swallowed_capacity_refusal_roll_back_actual_add() {
    use crate::smartcontracts::isi::triggers::set::SetReadOnly;
    use iroha_test_samples::ALICE_ID;
    use mv::storage::StorageReadOnly;
    for after_capture_refusal in [true, false] {
        let (state, old_id) = support::state_fixture(None, true);
        let mut block = state.block(support::header());
        register_journal_accounts(&mut block);
        let id: TriggerId = "journal_register_backing".parse().unwrap();
        let pool = AllocationBudget::new(if after_capture_refusal {
            std::mem::size_of::<OwnedTimeRetryTransition>()
        } else {
            1_000_000
        });
        let journal = ApplyingTimeRetryJournal::new(
            OutputTransaction::new(&mut block).unwrap(),
            &pool,
            1,
            1,
            10_000_000,
            support::limits(),
        )
        .unwrap();
        let result = if after_capture_refusal {
            // Before is absent and allocates no source bytes; the real Register
            // succeeds before its actual after-ID allocation is refused.
            journal.register_time(
                &ALICE_ID,
                register_journal_time_instruction(&id, 1, None, None),
            )
        } else {
            journal.with_scope(|scope| {
                scope.register_time(
                    &ALICE_ID,
                    register_journal_time_instruction(&id, 1, None, None),
                )?;
                assert!(
                    scope
                        .journal
                        .output
                        .transaction
                        .as_ref()
                        .unwrap()
                        .world
                        .triggers
                        .time_triggers()
                        .get(&id)
                        .is_some()
                );
                assert!(scope.set_retry_state(&id, value(1, 20)).is_err());
                Ok(())
            })
        };
        if after_capture_refusal {
            assert!(matches!(
                result,
                Err(TimeRetryJournalError::Source(
                    ApplyingTimeSourceError::IdBacking(_)
                ))
            ));
        } else {
            assert!(matches!(result, Err(TimeRetryJournalError::RecordCapacity)));
        }
        drop(result);
        assert_eq!(pool.reserved_bytes(), 0);
        assert_eq!(support::retry(&block, &old_id), None);
        assert!(
            block
                .world
                .triggers
                .triggers_iter()
                .all(|row| row.id() != &id)
        );
    }
}

#[test]
fn time_register_journal_caught_nested_unwind_consumes_original_add_overlay() {
    use crate::smartcontracts::isi::triggers::set::SetReadOnly;
    use iroha_test_samples::ALICE_ID;
    let (state, old_id) = support::state_fixture(None, true);
    let mut block = state.block(support::header());
    register_journal_accounts(&mut block);
    let id: TriggerId = "journal_register_unwind".parse().unwrap();
    let pool = AllocationBudget::new(1_000_000);
    let result = ApplyingTimeRetryJournal::new(
        OutputTransaction::new(&mut block).unwrap(),
        &pool,
        1,
        2,
        10_000_000,
        support::limits(),
    )
    .unwrap()
    .with_scope(|scope| {
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scope.with_scope(|inner| {
                inner.register_time(
                    &ALICE_ID,
                    register_journal_time_instruction(&id, 1, None, None),
                )?;
                panic!("unwind after actual Register in nested scope");
            })
        }));
        assert!(caught.is_err());
        Ok(())
    });
    assert!(matches!(result, Err(TimeRetryJournalError::ScopeAborted)));
    drop(result);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::retry(&block, &old_id), None);
    assert!(
        block
            .world
            .triggers
            .triggers_iter()
            .all(|row| row.id() != &id)
    );
}

#[test]
fn time_register_journal_non_time_input_refuses_without_dispatch_and_rolls_back_prior_retry() {
    use crate::smartcontracts::isi::triggers::set::SetReadOnly;
    use iroha_test_samples::ALICE_ID;
    let (state, old_id) = support::state_fixture(None, true);
    let mut block = state.block(support::header());
    register_journal_accounts(&mut block);
    let id: TriggerId = "journal_register_not_time".parse().unwrap();
    let pool = AllocationBudget::new(1_000_000);
    let result = ApplyingTimeRetryJournal::new(
        OutputTransaction::new(&mut block).unwrap(),
        &pool,
        2,
        1,
        10_000_000,
        support::limits(),
    )
    .unwrap()
    .set_retry_state(&old_id, value(1, 20))
    .unwrap()
    .register_time(&ALICE_ID, register_journal_by_call_instruction(&id));
    assert!(matches!(
        result,
        Err(TimeRetryJournalError::NonTimeRegistration)
    ));
    drop(result);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(support::retry(&block, &old_id), None);
    assert!(
        block
            .world
            .triggers
            .triggers_iter()
            .all(|row| row.id() != &id)
    );
}
