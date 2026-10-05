//! Independent local work boundaries and actual borrowed scalar allocation observations.
use super::*;
use iroha_allocation::AllocationBudget;
fn strategy(amount: u64, pool: &AllocationBudget) -> CheckedStrategy {
    CheckedStrategy::action_test_work(amount, pool)
}
fn bits(action: &LoadedAction<DataEventFilter>) -> ActionBits<'_> {
    ActionBits {
        repeats: &action.repeats,
        metadata: &action.metadata,
    }
}
#[test]
fn depleted_and_empty_metadata_below_exact_above_capsules_do_not_read_later_values() {
    let pool = AllocationBudget::new(0);
    let mut action = super::tests::action();
    action.repeats = Repeats::Exactly(0);
    for amount in [2, 3, 4] {
        let mut work = strategy(amount, &pool);
        assert_eq!(
            enabled(bits(&action), &mut work),
            if amount == 2 {
                Err(TriggerContractError::WorkLimit)
            } else {
                Ok(false)
            }
        );
    }
    action.repeats = Repeats::Indefinitely;
    for amount in [17, 18, 19] {
        let mut work = strategy(amount, &pool);
        assert_eq!(
            enabled(bits(&action), &mut work),
            if amount == 17 {
                Err(TriggerContractError::WorkLimit)
            } else {
                Ok(true)
            }
        );
    }
    assert_eq!(pool.peak_reserved_bytes(), 0);
    assert_eq!(pool.reserved_bytes(), 0);
}
#[test]
fn bool_and_u64_fallback_capsules_pay_the_complete_original_text_before_decode() {
    let pool = AllocationBudget::new(0);
    // one key:3+1+2*3+4+9+9+1+1+11=45 including shared helper11; bool false122+16*5=202; u64 zero138+124=262.
    for (value, units, expected) in [
        (iroha_primitives::json::Json::from(false), 247_u64, false),
        (iroha_primitives::json::Json::from(0_u64), 307, false),
    ] {
        let mut action = super::tests::action();
        action.metadata.insert("__enabled".parse().unwrap(), value);
        for amount in [units - 1, units, units + 1] {
            let mut work = strategy(amount, &pool);
            let actual = enabled(bits(&action), &mut work);
            assert_eq!(
                actual,
                if amount < units {
                    Err(TriggerContractError::WorkLimit)
                } else {
                    Ok(expected)
                }
            );
            if amount >= units {
                assert_eq!(work.action_remaining(), amount - units);
            }
        }
    }
    assert_eq!(pool.reserved_bytes(), 0);
}
#[test]
fn complete_metadata_tail_is_paid_even_when_enabled_key_matched_first() {
    let pool = AllocationBudget::new(0);
    let mut action = super::tests::action();
    action.metadata.insert("__enabled".parse().unwrap(), false);
    action.metadata.insert("z_tail".parse().unwrap(), true);
    // false247 plus next3+key4+6+9+branch1=23 =>270.
    for amount in [269, 270, 271] {
        let mut work = strategy(amount, &pool);
        assert_eq!(
            enabled(bits(&action), &mut work),
            if amount < 270 {
                Err(TriggerContractError::WorkLimit)
            } else {
                Ok(false)
            }
        );
    }
    assert_eq!(pool.peak_reserved_bytes(), 0);
}
#[test]
fn full_utf8_trigger_id_comparison_pays_both_original_name_operands() {
    let pool = AllocationBudget::new(0);
    let left: TriggerId = "雪".parse().unwrap();
    let right: TriggerId = "雪x".parse().unwrap();
    //7 fixed events+3 left+4 right=14, distinct original operands.
    for amount in [13, 14, 15] {
        let mut work = strategy(amount, &pool);
        assert_eq!(
            work.compare_action_ids(&left, &right),
            if amount == 13 {
                Err(TriggerContractError::WorkLimit)
            } else {
                Ok(false)
            }
        );
    }
    assert_eq!(pool.reserved_bytes(), 0);
}
#[test]
fn actual_history_merge_includes_current_masks_undo_some_and_absent_terminal_tails() {
    use mv::storage::Storage;
    let mut rows: Storage<TriggerId, usize> =
        [("a".parse().unwrap(), 1), ("c".parse().unwrap(), 3)]
            .into_iter()
            .collect();
    let mut block = rows.block();
    block.insert("a".parse().unwrap(), 10);
    block.remove("c".parse().unwrap());
    block.insert("b".parse().unwrap(), 2);
    block.remove("z".parse().unwrap());
    block.commit();
    let expected: Vec<_> = rows
        .history()
        .iter_before_block()
        .map(|(key, value)| (key.clone(), *value))
        .collect();
    let view = rows.try_committed_view_nonblocking().unwrap();
    let pool = AllocationBudget::new(0);
    let mut work = strategy(u64::MAX, &pool);
    let mut actual = Vec::new();
    visit_original(&view, Image::Predecessor, &mut work, |key, value, _| {
        actual.push((key.clone(), *value));
        Ok(())
    })
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 2);
    assert!(view.undo().iter().any(|(_, value)| value.is_none()));
    assert_eq!(pool.reserved_bytes(), 0);
}
#[test]
fn cold_and_warm_shipping_borrowed_bool_u64_and_malformed_scalar_helpers_do_not_allocate() {
    use crate::test_allocations;
    let values = [
        iroha_primitives::json::Json::from(false),
        iroha_primitives::json::Json::from(1_u64),
        iroha_primitives::json::Json::from("wrong"),
    ];
    let join = std::thread::spawn(move || {
        for _ in 0..2 {
            for (index, value) in values.iter().enumerate() {
                let mut actual = None;
                let allocations = test_allocations::allocations_during(|| {
                    actual = Some(super::super::super::trigger_enabled_from_value(
                        Some(value),
                        |_, _| Ok::<_, core::convert::Infallible>(()),
                    ))
                });
                assert_eq!(actual, Some(Ok(index == 1)));
                assert_eq!(allocations, 0);
            }
        }
        let positive = test_allocations::allocations_during(|| {
            let original = Box::new(31_u64);
            std::hint::black_box(&original);
        });
        assert_eq!(positive, 1);
    });
    join.join().unwrap();
}

#[test]
fn actual_masked_native_history_below_exact_above_includes_the_terminal_pass() {
    use mv::storage::Storage;
    let mut rows: Storage<TriggerId, usize> =
        [("a".parse().unwrap(), 1), ("c".parse().unwrap(), 3)]
            .into_iter()
            .collect();
    let mut block = rows.block();
    block.insert("a".parse().unwrap(), 10);
    block.remove("c".parse().unwrap());
    block.insert("b".parse().unwrap(), 2);
    block.remove("z".parse().unwrap());
    block.commit();
    let view = rows.try_committed_view_nonblocking().unwrap();
    assert_eq!(view.current().len(), 2);
    assert_eq!(view.undo().len(), 4);
    let expected: Vec<_> = rows
        .history()
        .iter_before_block()
        .map(|(key, value)| (key.clone(), *value))
        .collect();
    let exact = 2 * (29 * usize::BITS as u64 + 44) + 8 * (17 * usize::BITS as u64 + 26) + 42; //5 head passes*4 +4 Some/None branches +2 complete a/b comparisons9
    let pool = AllocationBudget::new(0);
    for amount in [exact - 1, exact, exact + 1] {
        let mut work = strategy(amount, &pool);
        let mut actual = Vec::new();
        let result = visit_original(&view, Image::Predecessor, &mut work, |key, value, _| {
            actual.push((key.clone(), *value));
            Ok(())
        });
        assert_eq!(actual, expected); // one-below has visited real rows, but cannot admit the terminal control pass
        assert_eq!(
            result,
            if amount < exact {
                Err(TriggerContractError::WorkLimit)
            } else {
                Ok(())
            }
        );
        if amount >= exact {
            assert_eq!(work.action_remaining(), amount - exact);
        }
    }
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn cold_and_warm_complete_borrowed_metadata_scan_and_scalar_fallback_do_not_allocate() {
    use crate::test_allocations;
    let mut action = super::tests::action();
    action.metadata.insert("__enabled".parse().unwrap(), 1_u64);
    action.metadata.insert("z_tail".parse().unwrap(), false);
    let join = std::thread::spawn(move || {
        let pool = AllocationBudget::new(0);
        let mut work = strategy(u64::MAX, &pool);
        for _ in 0..2 {
            let mut actual = None;
            let allocations = test_allocations::allocations_during(|| {
                actual = Some(enabled(bits(&action), &mut work))
            });
            assert_eq!(actual, Some(Ok(true)));
            assert_eq!(allocations, 0);
        }
        assert_eq!(pool.reserved_bytes(), 0);
        assert_eq!(pool.peak_reserved_bytes(), 0);
        let positive = test_allocations::allocations_during(|| {
            let actual = Box::new(41_u64);
            std::hint::black_box(&actual);
        });
        assert_eq!(positive, 1);
    });
    join.join().unwrap();
}

#[test]
fn independent_true_null_maximum_and_utf8_metadata_tail_literals_keep_scalar_parity() {
    use iroha_primitives::json::Json;
    let pool = AllocationBudget::new(0);
    for (value, text, units, expected) in [
        (Json::from(true), "true", 231_u64, true),
        (Json::from_str_norito("null").unwrap(), "null", 409, false),
        (Json::from(u64::MAX), "18446744073709551615", 953, true),
    ] {
        assert_eq!(value.get(), text);
        let mut action = super::tests::action();
        action.metadata.insert("__enabled".parse().unwrap(), value);
        for amount in [units - 1, units, units + 1] {
            let mut work = strategy(amount, &pool);
            assert_eq!(
                enabled(bits(&action), &mut work),
                if amount < units {
                    Err(TriggerContractError::WorkLimit)
                } else {
                    Ok(expected)
                }
            );
        }
    }
    let mut action = super::tests::action();
    action.metadata.insert("__enabled".parse().unwrap(), false);
    action.metadata.insert("a".parse().unwrap(), true);
    action.metadata.insert("雪".parse().unwrap(), true);
    // False247 plus two extra physical next calls6 and complete ASCII/UTF8 Names15+17 =285.
    for amount in [284, 285, 286] {
        let mut work = strategy(amount, &pool);
        assert_eq!(
            enabled(bits(&action), &mut work),
            if amount < 285 {
                Err(TriggerContractError::WorkLimit)
            } else {
                Ok(false)
            }
        );
    }
    assert_eq!(pool.reserved_bytes(), 0);
}
#[test]
fn exact_first_attempt_refusal_is_not_malformed_json_or_permission_to_try_u64() {
    let value = iroha_primitives::json::Json::from(1_u64);
    let calls = std::cell::Cell::new(0);
    let result = super::super::super::trigger_enabled_from_value(Some(&value), |scalar, _| {
        calls.set(calls.get() + 1);
        assert!(matches!(scalar, super::super::super::EnabledScalar::Bool));
        Err::<(), _>(TriggerContractError::WorkLimit)
    });
    assert_eq!(result, Err(TriggerContractError::WorkLimit));
    assert_eq!(calls.get(), 1);
    let calls = std::cell::Cell::new(0);
    let result = super::super::super::trigger_enabled_from_value(None, |_, _| {
        calls.set(calls.get() + 1);
        Err::<(), _>(TriggerContractError::WorkLimit)
    });
    assert_eq!(result, Ok(true));
    assert_eq!(calls.get(), 0);
}
