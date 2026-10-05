//! Single original layer, shared cumulative counters and scoped reinstatement.

use super::*;

fn bytes_limit(bytes: usize) -> DecodeLimits {
    DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
}
fn active_count() -> usize {
    budget_scope::with_active(|layers| layers.len())
}

#[test]
fn owned_context_admits_native_control_and_retains_it_through_the_last_clone() {
    let layout = DecodeBudgetContext::allocation_layout();
    let short = iroha_allocation::AllocationBudget::new(layout.size() - 1);
    assert!(DecodeBudgetContext::try_new_owned(bytes_limit(8), &short).is_err());
    assert_eq!(short.reserved_bytes(), 0);
    let pool = iroha_allocation::AllocationBudget::new(layout.size());
    let original = DecodeBudgetContext::try_new_owned(bytes_limit(8), &pool).unwrap();
    let cloned = original.clone();
    assert_eq!(pool.reserved_bytes(), layout.size());
    original
        .with(|| cloned.with(|| reserve_decode_allocation(8)))
        .unwrap();
    assert_eq!(cloned.consumed_allocated_bytes(), 8);
    drop(original);
    assert_eq!(pool.reserved_bytes(), layout.size());
    assert!(cloned.with(|| reserve_decode_allocation(1)).is_err());
    drop(cloned);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn cloned_context_deduplicates_only_its_original_counter_identity() {
    let original = DecodeBudgetContext::new(bytes_limit(8));
    let cloned = original.clone();
    let independent = DecodeBudgetContext::new(bytes_limit(8));
    assert!(
        original
            .layer
            .budget
            .counters
            .ptr_eq(&cloned.layer.budget.counters)
    );
    assert!(
        !original
            .layer
            .budget
            .counters
            .ptr_eq(&independent.layer.budget.counters)
    );
    let baseline = active_count();
    original.with(|| {
        cloned.with(|| {
            assert_eq!(active_count(), baseline + 1);
            reserve_decode_allocation(4).unwrap();
            assert_eq!(
                original
                    .layer
                    .budget
                    .counters
                    .total_allocated_bytes
                    .load(Ordering::Relaxed),
                4
            );
            independent.with(|| {
                assert_eq!(active_count(), baseline + 2);
                reserve_decode_allocation(4).unwrap();
                assert_eq!(
                    original
                        .layer
                        .budget
                        .counters
                        .total_allocated_bytes
                        .load(Ordering::Relaxed),
                    8
                );
                assert_eq!(
                    independent
                        .layer
                        .budget
                        .counters
                        .total_allocated_bytes
                        .load(Ordering::Relaxed),
                    4
                );
            });
        })
    });
    assert_eq!(active_count(), baseline);
}

#[test]
fn escaped_context_retains_cumulative_consumption_after_original_scope_and_owner_end() {
    let escaped = {
        let original = DecodeBudgetContext::new(bytes_limit(8));
        original.with(|| {
            reserve_decode_allocation(3).unwrap();
            original.clone()
        })
    };
    escaped.with(|| {
        reserve_decode_allocation(5).unwrap();
        assert!(matches!(
            reserve_decode_allocation(1),
            Err(Error::TotalAllocationExceeded {
                attempted: 9,
                limit: 8
            })
        ));
    });
}

#[test]
fn reinstating_context_keeps_ambient_limits_and_restores_layer_membership() {
    let escaped = DecodeBudgetContext::new(bytes_limit(8));
    let baseline = active_count();
    with_decode_limits_scope(bytes_limit(5), || {
        escaped.with(|| {
            reserve_decode_allocation(3).unwrap();
            assert!(matches!(
                reserve_decode_allocation(3),
                Err(Error::TotalAllocationExceeded {
                    attempted: 6,
                    limit: 5
                })
            ));
        })
    });
    assert_eq!(active_count(), baseline);
    escaped.with(|| {
        // Refusing the original outer layer did not charge a later layer or reset it.
        reserve_decode_allocation(5).unwrap();
        assert!(matches!(
            reserve_decode_allocation(1),
            Err(Error::TotalAllocationExceeded {
                attempted: 9,
                limit: 8
            })
        ));
    });
}

#[test]
fn escaped_context_preserves_its_base_depth_and_restores_callers_depth() {
    let baseline = DECODE_NESTING_DEPTH.with(Cell::get);
    let escaped = {
        let _depth = DecodeDepthGuard::enter().unwrap();
        DecodeBudgetContext::new(DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            1,
        ))
    };
    assert_eq!(DECODE_NESTING_DEPTH.with(Cell::get), baseline);
    escaped.with(|| {
        assert_eq!(DECODE_NESTING_DEPTH.with(Cell::get), baseline + 1);
        let _depth = DecodeDepthGuard::enter().unwrap();
        let same = escaped.clone();
        same.with(|| {
            assert_eq!(DECODE_NESTING_DEPTH.with(Cell::get), baseline + 2);
            assert!(matches!(
                DecodeDepthGuard::enter(),
                Err(Error::NestingDepthExceeded {
                    depth: 2,
                    limit: 1,
                    context: "decode budget"
                })
            ));
        });
    });
    assert_eq!(DECODE_NESTING_DEPTH.with(Cell::get), baseline);
}

#[test]
fn cloned_context_on_another_thread_shares_the_original_cumulative_counter() {
    let original = DecodeBudgetContext::new(bytes_limit(8));
    original.with(|| reserve_decode_allocation(3).unwrap());
    let moved = original.clone();
    std::thread::spawn(move || moved.with(|| reserve_decode_allocation(5)))
        .join()
        .unwrap()
        .unwrap();
    original.with(|| {
        assert!(matches!(
            reserve_decode_allocation(1),
            Err(Error::TotalAllocationExceeded {
                attempted: 9,
                limit: 8
            })
        ))
    });
}

#[test]
fn cloned_context_unwind_restores_enclosing_scope_without_resetting_counters() {
    let original = DecodeBudgetContext::new(bytes_limit(8));
    let baseline = active_count();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let clone = original.clone();
        clone.with(|| {
            reserve_decode_allocation(3).unwrap();
            panic!("scope operation unwinds");
        });
    }));
    assert!(result.is_err());
    assert_eq!(active_count(), baseline);
    original.with(|| {
        reserve_decode_allocation(5).unwrap();
        assert!(matches!(
            reserve_decode_allocation(1),
            Err(Error::TotalAllocationExceeded {
                attempted: 9,
                limit: 8
            })
        ));
    });
}

#[test]
fn prepaid_context_short_reservation_preserves_original_credit_despite_spare_pool_capacity() {
    let layout = DecodeBudgetContext::allocation_layout();
    let pool = iroha_allocation::AllocationBudget::new(2 * layout.size());
    let mut short = pool.try_reserve_bytes(layout.size() - 1).unwrap();
    let error = DecodeBudgetContext::from_reservation(bytes_limit(8), &mut short)
        .err()
        .expect("the original remainder cannot fund the exact counter control");
    assert_eq!(
        error,
        iroha_allocation::PrepaidSharedError::Reservation(
            iroha_allocation::InsufficientReservation {
                requested_bytes: layout.size(),
                remaining_bytes: layout.size() - 1,
            }
        )
    );
    assert_eq!(short.remaining_bytes(), layout.size() - 1);
    assert_eq!(pool.reserved_bytes(), layout.size() - 1);
    assert!(short.belongs_to(&pool));
    let independent = pool.try_reserve(layout).unwrap();
    assert_eq!(pool.reserved_bytes(), 2 * layout.size() - 1);
    drop(independent);
    assert_eq!(short.remaining_bytes(), layout.size() - 1);
    assert_eq!(pool.reserved_bytes(), layout.size() - 1);
    drop(short);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn prepaid_context_keeps_original_pool_and_counters_through_pool_shrink_and_last_clone() {
    let layout = DecodeBudgetContext::allocation_layout();
    let unused = 17;
    let sibling_bytes = 3;
    let admitted = layout.size() + unused + sibling_bytes;
    let pool = iroha_allocation::AllocationBudget::new(admitted);
    let foreign = iroha_allocation::AllocationBudget::new(admitted);
    let sibling = pool.try_reserve_bytes(sibling_bytes).unwrap();
    let mut reservation = pool.try_reserve_bytes(layout.size() + unused).unwrap();
    pool.set_limit_bytes(0);
    assert!(matches!(
        pool.try_reserve_bytes(1),
        Err(iroha_allocation::AllocationRefusal::ExceedsLimit {
            requested_bytes: 1,
            limit_bytes: 0
        })
    ));
    let original = DecodeBudgetContext::from_reservation(bytes_limit(8), &mut reservation).unwrap();
    let cloned = original.clone();
    assert_eq!(reservation.remaining_bytes(), unused);
    assert_eq!(pool.reserved_bytes(), admitted);
    assert!(
        original
            .layer
            .budget
            .counters
            .ptr_eq(&cloned.layer.budget.counters)
    );
    for context in [&original, &cloned] {
        let CounterOwner::Prepared(counters) = &context.layer.budget.counters else {
            panic!("prepaid construction retains its actual charged counter control");
        };
        assert!(counters.belongs_to(&pool));
        assert!(!counters.belongs_to(&foreign));
    }
    let baseline = active_count();
    original.with(|| {
        cloned.with(|| {
            assert_eq!(active_count(), baseline + 1);
            reserve_decode_allocation(3).unwrap();
        });
    });
    assert_eq!(active_count(), baseline);
    assert_eq!(cloned.consumed_allocated_bytes(), 3);
    drop(reservation);
    assert_eq!(pool.reserved_bytes(), layout.size() + sibling_bytes);
    drop(original);
    assert_eq!(pool.reserved_bytes(), layout.size() + sibling_bytes);
    cloned.with(|| reserve_decode_allocation(5)).unwrap();
    assert_eq!(cloned.consumed_allocated_bytes(), 8);
    assert!(matches!(
        cloned.with(|| reserve_decode_allocation(1)),
        Err(Error::TotalAllocationExceeded {
            attempted: 9,
            limit: 8
        })
    ));
    drop(sibling);
    assert_eq!(pool.reserved_bytes(), layout.size());
    drop(cloned);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(foreign.reserved_bytes(), 0);
    assert_eq!(pool.limit_bytes(), 0);
}
