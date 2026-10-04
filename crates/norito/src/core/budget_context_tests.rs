//! Single original layer, shared cumulative counters and scoped reinstatement.

use super::*;

fn bytes_limit(bytes: usize) -> DecodeLimits {
    DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
}
fn active_count() -> usize {
    budget_scope::with_active(|layers| layers.len())
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
