//! Original prepared controls, limit precedence, attempt identity and unwind tests.

use super::*;

fn limit(bytes: usize) -> DecodeLimits {
    DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
}
fn prepared(pool: &AllocationBudget) -> PreparedDecodeWorkspace {
    let mut reservation = pool
        .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
        .unwrap();
    PreparedDecodeWorkspace::from_reservation(pool, &mut reservation).unwrap()
}
fn demand() -> usize {
    PreparedDecodeWorkspace::allocation_layouts()
        .iter()
        .map(Layout::size)
        .sum()
}

#[test]
fn aggregate_preparation_refuses_foreign_or_short_original_remainder_before_consumption() {
    let pool = AllocationBudget::new(demand());
    let foreign = AllocationBudget::new(demand());
    let mut reservation = pool
        .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
        .unwrap();
    assert!(matches!(
        PreparedDecodeWorkspace::from_reservation(&foreign, &mut reservation),
        Err(PreparedDecodeScopeError::ForeignPool)
    ));
    assert_eq!(reservation.remaining_bytes(), demand());
    let held = reservation.try_split(Layout::new::<u8>()).unwrap();
    assert!(
        matches!(PreparedDecodeWorkspace::from_reservation(&pool, &mut reservation),
        Err(PreparedDecodeScopeError::Reservation(InsufficientReservation { requested_bytes, remaining_bytes })) if requested_bytes==demand() && remaining_bytes==demand()-1)
    );
    assert_eq!(reservation.remaining_bytes(), demand() - 1);
    drop((reservation, held));
    let workspace = prepared(&pool);
    assert!(workspace.belongs_to(&pool));
    assert!(!workspace.belongs_to(&foreign));
    assert_eq!(pool.reserved_bytes(), demand());
    drop(workspace);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn prepared_scope_uses_original_enclosing_limits_and_retains_control_through_error_lifetime() {
    let pool = AllocationBudget::new(demand());
    let mut workspace = prepared(&pool);
    pool.set_limit_bytes(0);
    let error = with_decode_limits_scope(limit(0), || {
        classify_decode_attempt(|| {
            workspace
                .with_limits(limit(8), limit(8), || reserve_decode_allocation(1))
                .unwrap()
        })
    })
    .unwrap_err();
    assert_eq!(error.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    assert_eq!(
        error.into_error().decode_resource_error(),
        Some(DecodeResourceError::TotalAllocationExceeded {
            attempted: 1,
            limit: 0
        })
    );
    let error = with_decode_limits_scope(limit(0), || {
        classify_decode_attempt(|| {
            workspace
                .with_limits(limit(8), limit(8), || reserve_decode_allocation(1))
                .unwrap()
        })
    })
    .unwrap_err();
    drop(workspace);
    assert_eq!(
        pool.reserved_bytes(),
        PreparedDecodeWorkspace::allocation_layouts()[0].size()
    );
    assert_eq!(error.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    drop(error);
    assert_eq!(pool.reserved_bytes(), 0);
    assert!(!decode_limits_active());
}

#[test]
fn reused_workspace_cannot_launder_old_error_into_new_attempt_and_protocol_limits_win() {
    let pool = AllocationBudget::new(demand());
    let mut workspace = prepared(&pool);
    let original = with_decode_limits_scope(limit(0), || {
        classify_decode_attempt(|| {
            workspace
                .with_limits(limit(8), limit(8), || reserve_decode_allocation(1))
                .unwrap()
        })
    })
    .unwrap_err();
    let stale = original.into_error();
    let replayed = with_decode_limits_scope(limit(0), || {
        classify_decode_attempt(|| {
            workspace
                .with_limits(limit(8), limit(8), || Err::<(), _>(stale))
                .unwrap()
        })
    })
    .unwrap_err();
    assert_eq!(replayed.kind(), DecodeAttemptErrorKind::Invalid);
    let intrinsic = with_decode_limits_scope(limit(0), || {
        classify_decode_attempt(|| {
            workspace
                .with_limits(limit(0), limit(0), || reserve_decode_allocation(1))
                .unwrap()
        })
    })
    .unwrap_err();
    assert_eq!(intrinsic.kind(), DecodeAttemptErrorKind::Invalid);
    workspace
        .with_limits(limit(8), limit(8), || reserve_decode_allocation(8))
        .unwrap()
        .unwrap();
    drop((workspace, replayed, intrinsic));
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn one_scope_chain_deduplicates_reapplied_context_and_restores_on_unwind() {
    let context = DecodeBudgetContext::new(limit(5));
    context
        .with(|| context.with(|| reserve_decode_allocation(3)))
        .unwrap();
    assert_eq!(
        context
            .layer
            .budget
            .counters
            .total_allocated_bytes
            .load(Ordering::Relaxed),
        3
    );
    assert!(!decode_limits_active());
    let pool = AllocationBudget::new(demand());
    let mut workspace = prepared(&pool);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        context.with(|| {
            workspace.with_limits(limit(8), limit(8), || {
                reserve_decode_allocation(2).unwrap();
                panic!("prepared scope interrupted");
            })
        })
    }));
    assert!(caught.is_err());
    assert!(!decode_limits_active());
    assert_eq!(DECODE_NESTING_DEPTH.with(Cell::get), 0);
    workspace
        .with_limits(limit(8), limit(8), || reserve_decode_allocation(8))
        .unwrap()
        .unwrap();
    workspace.attempt = u64::MAX;
    let mut called = false;
    assert!(matches!(
        workspace.with_limits(limit(8), limit(8), || called = true),
        Err(PreparedDecodeScopeError::AttemptExhausted)
    ));
    assert!(!called);
    assert!(!decode_limits_active());
    drop(workspace);
    assert_eq!(pool.reserved_bytes(), 0);
}
