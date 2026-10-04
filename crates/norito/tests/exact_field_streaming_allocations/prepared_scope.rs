//! Actual original scope-control allocation, refusal and reuse measurements.

use iroha_allocation::{AllocationBudget, PrepaidSharedError};
use norito::core::{
    DecodeAttemptErrorKind, DecodeLimits, PreparedDecodeScopeError, PreparedDecodeWorkspace,
    classify_decode_attempt, decode_limits_active, reserve_decode_allocation,
    with_decode_limits_scope,
};

fn demand() -> usize {
    PreparedDecodeWorkspace::allocation_layouts()
        .iter()
        .map(std::alloc::Layout::size)
        .sum()
}
fn limit(bytes: usize) -> DecodeLimits {
    DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
}
fn prepare(pool: &AllocationBudget) -> PreparedDecodeWorkspace {
    let mut reservation = pool
        .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
        .unwrap();
    PreparedDecodeWorkspace::from_reservation(pool, &mut reservation).unwrap()
}
fn refusing<R>(size: usize, successes: usize, body: impl FnOnce() -> R) -> R {
    struct Restore(usize, usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            super::REFUSE_SIZE.set(self.0);
            super::MATCHES_BEFORE_REFUSAL.set(self.1);
        }
    }
    let _restore = Restore(
        super::REFUSE_SIZE.replace(size),
        super::MATCHES_BEFORE_REFUSAL.replace(successes),
    );
    body()
}

#[test]
fn prepared_scope_admits_two_real_controls_and_reuses_them_without_allocator_calls() {
    let pool = AllocationBudget::new(demand());
    let mut reservation = pool
        .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
        .unwrap();
    let mut result = None;
    let allocations = super::allocations_during(|| {
        result = Some(PreparedDecodeWorkspace::from_reservation(
            &pool,
            &mut reservation,
        ));
    });
    assert_eq!(
        allocations, 2,
        "both actual shared controls are original-funded"
    );
    let mut workspace = result.unwrap().unwrap();
    assert_eq!(reservation.remaining_bytes(), 0);
    pool.set_limit_bytes(0);
    let allocations = super::allocations_during(|| {
        for _ in 0..32 {
            classify_decode_attempt(|| {
                workspace
                    .with_limits(limit(8), limit(8), || reserve_decode_allocation(8))
                    .unwrap()
            })
            .unwrap();
        }
    });
    assert_eq!(
        allocations, 0,
        "enter/reuse/observe/retire must reuse both controls"
    );
    assert_eq!(pool.reserved_bytes(), demand());
    drop((workspace, reservation));
    assert_eq!(pool.reserved_bytes(), 0);
    assert!(!decode_limits_active());
}

#[test]
fn original_scope_refusal_classifies_without_allocating_after_workspace_admission() {
    let pool = AllocationBudget::new(demand());
    let mut workspace = prepare(&pool);
    pool.set_limit_bytes(0);
    let mut error = None;
    // Enclosing caller owns its existing scope before the measured attempt.
    let allocations = with_decode_limits_scope(limit(0), || {
        super::allocations_during(|| {
            error = Some(
                classify_decode_attempt(|| {
                    workspace
                        .with_limits(limit(8), limit(8), || reserve_decode_allocation(1))
                        .unwrap()
                })
                .unwrap_err(),
            );
        })
    });
    assert_eq!(allocations, 0);
    assert_eq!(
        error.as_ref().unwrap().kind(),
        DecodeAttemptErrorKind::EnclosingLimit
    );
    drop(workspace);
    assert_eq!(
        pool.reserved_bytes(),
        PreparedDecodeWorkspace::allocation_layouts()[0].size(),
        "captured error is the final original control owner after scope retirement"
    );
    drop(error);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn original_prepaid_scope_allocator_failure_refunds_only_constructed_controls() {
    let layout = PreparedDecodeWorkspace::allocation_layouts()[0];
    for successful_controls in [0, 1] {
        let pool = AllocationBudget::new(demand() + 17);
        let mut reservation = pool.try_reserve_bytes(demand() + 17).unwrap();
        let error = refusing(layout.size(), successful_controls, || {
            PreparedDecodeWorkspace::from_reservation(&pool, &mut reservation)
        })
        .err()
        .expect("actual original shared control allocator refusal");
        assert!(matches!(
            error,
            PreparedDecodeScopeError::Allocation(PrepaidSharedError::Allocator { requested_bytes })
                if requested_bytes == layout.size()
        ));
        let remaining = demand() + 17 - layout.size() * (successful_controls + 1);
        assert_eq!(reservation.remaining_bytes(), remaining);
        assert_eq!(pool.reserved_bytes(), remaining);
        drop(reservation);
        assert_eq!(pool.reserved_bytes(), 0);
        let workspace = prepare(&pool);
        assert!(workspace.belongs_to(&pool));
        drop(workspace);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}
