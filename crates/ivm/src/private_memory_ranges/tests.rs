//! Finite original-pool controls for interval growth, copies, reset and teardown.

use super::*;
use iroha_allocation::AllocationRefusal;

const ROW_BYTES: usize = std::mem::size_of::<(u64, u64)>();

#[test]
fn funded_growth_refusal_retains_ranges_and_allows_original_pool_retry() {
    let budget = AllocationBudget::new(4 * ROW_BYTES);
    let mut ranges = PrivateMemoryRanges::with_memory_budget(&budget);
    for index in 0..4 {
        ranges.try_insert(index * 4..index * 4 + 2).unwrap();
    }
    let original = ranges.pairs_for_testing().to_vec();
    assert_eq!(budget.reserved_bytes(), 4 * ROW_BYTES);
    assert!(matches!(
        ranges.try_insert(20..22),
        Err(VMError::AllocationDeferred(
            AllocationRefusal::ExceedsLimit { .. }
        ))
    ));
    assert_eq!(ranges.pairs_for_testing(), original);
    assert_eq!(budget.reserved_bytes(), 4 * ROW_BYTES);

    // Both the four-row original and eight-row replacement remain funded
    // until replacement construction and copying have completed.
    budget.set_limit_bytes(12 * ROW_BYTES - 1);
    assert!(matches!(
        ranges.try_insert(20..22),
        Err(VMError::AllocationDeferred(
            AllocationRefusal::Capacity { .. }
        ))
    ));
    assert_eq!(ranges.pairs_for_testing(), original);
    budget.set_limit_bytes(12 * ROW_BYTES);
    ranges.try_insert(20..22).unwrap();
    assert_eq!(budget.reserved_bytes(), 8 * ROW_BYTES);
    assert_eq!(ranges.pairs_for_testing().last(), Some(&(20, 22)));
    drop(ranges);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn funded_interval_operations_match_local_storage_after_splits_merges_and_removals() {
    let budget = AllocationBudget::new(128 * ROW_BYTES);
    let mut funded = PrivateMemoryRanges::with_memory_budget(&budget);
    let mut local = PrivateMemoryRanges::default();
    for (range, private) in [
        (20..40, true),
        (60..70, true),
        (10..15, true),
        (30..65, true),
        (22..24, false),
        (0..12, false),
        (23..68, false),
        (14..69, true),
        (11..71, false),
        (100..100, true),
    ] {
        funded.try_prepare_update(&range, private).unwrap();
        local.try_prepare_update(&range, private).unwrap();
        funded.apply_prepared_update(range.clone(), private);
        local.apply_prepared_update(range, private);
        assert_eq!(funded, local);
        for start in 0..100 {
            assert_eq!(
                funded.intersection_len(start..start + 9),
                local.intersection_len(start..start + 9)
            );
            assert_eq!(
                funded.intersects(start..start + 9),
                local.intersects(start..start + 9)
            );
        }
    }
    assert!(funded.is_empty());
    assert!(
        budget.reserved_bytes() > 0,
        "empty storage retains its real capacity"
    );
    drop(funded);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn template_copy_spends_original_parent_credit_and_rejects_an_equal_foreign_pool() {
    let budget = AllocationBudget::new(128 * ROW_BYTES);
    let mut source = PrivateMemoryRanges::with_memory_budget(&budget);
    source.try_insert(10..20).unwrap();
    let bytes = budget.reserved_bytes();
    let plan = source.runtime_template_memory_plan().unwrap();
    let foreign = AllocationBudget::new(128 * ROW_BYTES);
    let mut foreign_lease = ExecutionMemoryLease::reserve(&foreign, plan).unwrap();
    assert!(
        source
            .try_clone_for_runtime_template(Some(&mut foreign_lease))
            .is_err()
    );
    assert_eq!(foreign_lease.remaining_bytes(), ROW_BYTES);
    assert_eq!(budget.reserved_bytes(), bytes);
    assert!(source.try_clone_for_runtime_template(None).is_err());

    let mut lease = ExecutionMemoryLease::reserve(&budget, plan).unwrap();
    budget.set_limit_bytes(0);
    let copy = source
        .try_clone_for_runtime_template(Some(&mut lease))
        .unwrap();
    assert_eq!(lease.remaining_bytes(), 0);
    assert_eq!(copy, source);
    drop(lease);
    drop(source);
    assert_eq!(budget.reserved_bytes(), ROW_BYTES);
    assert!(matches!(
        copy.try_clone(),
        Err(VMError::AllocationDeferred(_))
    ));
    assert_eq!(copy.pairs_for_testing(), &[(10, 20)]);
    drop(copy);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn standalone_copy_and_unwind_keep_only_live_interval_charges() {
    let budget = AllocationBudget::new(128 * ROW_BYTES);
    let mut source = PrivateMemoryRanges::with_memory_budget(&budget);
    source.try_insert(10..20).unwrap();
    let bytes = budget.reserved_bytes();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let copied = source.try_clone().unwrap();
        assert_eq!(budget.reserved_bytes(), bytes + ROW_BYTES);
        assert_eq!(copied, source);
        panic!("abandon interval copy");
    }));
    assert!(outcome.is_err());
    assert_eq!(budget.reserved_bytes(), bytes);
    source.clear();
    source.try_insert(30..40).unwrap();
    assert_eq!(budget.reserved_bytes(), bytes);
    drop(source);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn physical_copy_refusal_returns_parent_credit_without_changing_intervals() {
    let budget = AllocationBudget::new(128 * ROW_BYTES);
    let mut source = PrivateMemoryRanges::with_memory_budget(&budget);
    source.try_insert(10..20).unwrap();
    let bytes = budget.reserved_bytes();
    PrivateMemoryRanges::refuse_next_copy_for_testing();
    assert!(source.try_clone().is_err());
    assert_eq!(budget.reserved_bytes(), bytes);
    assert_eq!(source.pairs_for_testing(), &[(10, 20)]);
    assert_eq!(source.try_clone().unwrap(), source);
    assert_eq!(budget.reserved_bytes(), bytes);
}

#[test]
fn restore_preflight_keeps_contents_on_refusal_then_reuses_capacity_at_zero_limit() {
    let budget = AllocationBudget::new(128 * ROW_BYTES);
    let mut template = PrivateMemoryRanges::with_memory_budget(&budget);
    for index in 0..5 {
        template.try_insert(index * 4..index * 4 + 2).unwrap();
    }
    let mut current = PrivateMemoryRanges::with_memory_budget(&budget);
    current.try_insert(100..110).unwrap();
    let bytes = budget.reserved_bytes();
    budget.set_limit_bytes(bytes);
    assert!(current.try_prepare_restore(&template).is_err());
    assert_eq!(budget.reserved_bytes(), bytes);
    assert_eq!(current.pairs_for_testing(), &[(100, 110)]);
    budget.set_limit_bytes(bytes + 8 * ROW_BYTES);
    current.try_prepare_restore(&template).unwrap();
    assert_eq!(
        current.pairs_for_testing(),
        &[(100, 110)],
        "preflight changes no privacy tags"
    );
    current.restore_prepared(&template);
    assert_eq!(current, template);
    let bytes = budget.reserved_bytes();
    budget.set_limit_bytes(0);
    current.clear();
    current.try_prepare_restore(&template).unwrap();
    current.restore_prepared(&template);
    assert_eq!(current, template);
    assert_eq!(budget.reserved_bytes(), bytes);
    drop(current);
    drop(template);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn empty_funded_intervals_need_no_credit_or_backing() {
    let budget = AllocationBudget::new(0);
    let source = PrivateMemoryRanges::with_memory_budget(&budget);
    let mut copy = source.try_clone().unwrap();
    assert_eq!(budget.reserved_bytes(), 0);
    copy.try_prepare_restore(&source).unwrap();
    copy.restore_prepared(&source);
    assert!(copy.is_empty());
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(copy.try_insert(1..2).is_err());
    assert!(copy.is_empty());
}

#[test]
fn retention_and_activation_never_refund_original_interval_credit() {
    use crate::ivm_cache::{CacheLimits, CacheLimitsGuard, configure_limits};

    let limits = CacheLimits {
        capacity: 4,
        max_bytes: usize::MAX,
        max_decoded_ops: 0,
    };
    let _limits = CacheLimitsGuard::new(limits);
    let budget = AllocationBudget::new(16 * ROW_BYTES);
    let mut funded = PrivateMemoryRanges::with_memory_budget(&budget);
    let mut local = PrivateMemoryRanges::default();
    funded.try_insert(10..20).unwrap();
    local.try_insert(10..20).unwrap();
    let bytes = budget.reserved_bytes();
    assert!(funded.try_retain());
    assert!(local.try_retain());
    funded.make_active();
    local.make_active();
    assert_eq!(budget.reserved_bytes(), bytes);
    configure_limits(CacheLimits {
        max_bytes: 0,
        ..limits
    });
    assert!(!funded.try_retain());
    assert!(!local.try_retain());
    assert_eq!(budget.reserved_bytes(), bytes);
    funded.try_insert(30..40).unwrap();
    assert_eq!(budget.reserved_bytes(), bytes);
    drop(funded);
    assert_eq!(budget.reserved_bytes(), 0);
}
