//! Atomic World control admission must retain the actual original refusal observation.

use super::*;
use crate::{state::World, test_allocations};
use iroha_allocation::{
    AllocationBudget, AllocationRefusal, OwnedAllocationScope, release::ReleaseRegistration,
};
use std::task::{Context, Poll, Waker};

fn block<'world>(
    world: &'world World,
    source: &AllocationBudget,
    replacement: bool,
) -> Result<WorldBlock<'world>, AdmittedStorageError> {
    if replacement {
        world.try_block_and_revert(source)
    } else {
        world.try_block(source)
    }
}

#[test]
fn world_control_capacity_refusal_stays_pending_until_original_external_release() {
    let _retirement = crossbeam_epoch::pin();
    for replacement in [false, true] {
        let source = AllocationBudget::new(16 << 20);
        let world = World::try_with_execution_budget([], [], [], &source).unwrap();
        let baseline = source.reserved_bytes();
        let scope_bytes = OwnedAllocationScope::allocation_layout().size();
        let aggregate = OriginalWorldFields::layout().size()
            + original_world_cell_control_bytes(&world)
            + scope_bytes;
        let mode = if replacement {
            BlockMode::Replace
        } else {
            BlockMode::Ordinary
        };
        let complete = OriginalWorldFields::layout().size()
            + original_world_cell_control_bytes(&world)
            + prepaid_world_table_writer_bytes(mode, false);
        assert!(complete >= aggregate);
        source.set_limit_bytes(baseline + complete);
        // Room for a refund scope alone, but one byte short of its complete
        // field inventory. An early scope would refund itself on this refusal.
        let external = source.try_reserve_bytes(complete - aggregate + 1).unwrap();
        let occupied = source.reserved_bytes();
        let watch = AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
        let mut registration = crate::unit_test_support::release_registration(&watch);
        registration.bind_thread(std::thread::current());
        let original = match block(&world, &source, replacement) {
            Err(error @ AdmittedStorageError::Allocation(AllocationRefusal::Capacity { .. })) => {
                error
            }
            Err(error) => panic!("expected original aggregate capacity refusal, got {error:?}"),
            Ok(_) => panic!("the original pool cannot fund the full World controls"),
        };
        let release = original.release_wait().expect("original pool observation");
        assert_eq!(
            registration.poll_wait(release, &mut Context::from_waker(Waker::noop())),
            Poll::Pending,
            "World control admission must not satisfy its own original release wait"
        );
        assert!(matches!(
            &original,
            AdmittedStorageError::Allocation(AllocationRefusal::Capacity { requested_bytes, .. })
                if *requested_bytes == aggregate
        ));
        assert_eq!(source.reserved_bytes(), occupied);

        let foreign = AllocationBudget::new(aggregate);
        drop(foreign.try_reserve_bytes(aggregate).unwrap());
        for _ in 0..3 {
            let mut again = None;
            let allocations = test_allocations::allocations_during(|| {
                again = Some(block(&world, &source, replacement));
            });
            match again.unwrap() {
                Err(error) => assert_eq!(error, original),
                Ok(_) => panic!("unchanged external occupancy still refuses"),
            }
            assert_eq!(
                allocations, 0,
                "refused aggregate must allocate no prefix control"
            );
            assert_eq!(source.reserved_bytes(), occupied);
            assert_eq!(
                registration.poll_wait(release, &mut Context::from_waker(Waker::noop())),
                Poll::Pending,
                "repeated refusal and foreign releases cannot replace the original observation"
            );
        }
        drop(external);
        assert_eq!(
            registration.poll_wait(release, &mut Context::from_waker(Waker::noop())),
            Poll::Ready(())
        );
        assert_eq!(source.reserved_bytes(), baseline);
        let acquired = block(&world, &source, replacement).unwrap();
        assert!(acquired.fields.as_ref().unwrap().belongs_to(&source));
        assert_eq!(source.reserved_bytes(), baseline + complete);
        drop(acquired);
        assert_eq!(source.reserved_bytes(), baseline);
    }
}

#[test]
fn world_scope_and_shell_physical_refusal_preserve_exact_layout_and_release_all_credit() {
    let _retirement = crossbeam_epoch::pin();
    for replacement in [false, true] {
        for layout in [
            OwnedAllocationScope::allocation_layout(),
            OriginalWorldFields::layout(),
        ] {
            let source = AllocationBudget::new(16 << 20);
            let world = World::try_with_execution_budget([], [], [], &source).unwrap();
            let sibling = source.try_reserve_bytes(17).unwrap();
            let baseline = source.reserved_bytes();
            let (result, observed) = test_allocations::refuse_one_layout_during(layout, || {
                block(&world, &source, replacement)
            });
            assert!(
                observed,
                "the exact original scope or shell allocation was reached"
            );
            let error = match result {
                Err(error) => error,
                Ok(_) => panic!("refused original physical backing cannot produce a World"),
            };
            assert_eq!(error, AdmittedStorageError::Allocator { layout });
            assert!(
                error.release_wait().is_none(),
                "allocator refusal is not a pool wait"
            );
            assert_eq!(
                source.reserved_bytes(),
                baseline,
                "all unused aggregate credits retire"
            );
            drop(block(&world, &source, replacement).unwrap());
            assert_eq!(source.reserved_bytes(), baseline);
            drop(sibling);
            assert_eq!(source.reserved_bytes(), baseline - 17);
        }
    }
}

#[test]
fn original_refund_scope_rejects_foreign_or_short_credit_without_consumption() {
    let layout = OwnedAllocationScope::allocation_layout();
    let source = AllocationBudget::new(layout.size());
    let foreign = AllocationBudget::new(layout.size());
    let mut reservation = source.try_reserve(layout).unwrap();
    assert!(matches!(
        original_refund_scope(&foreign, &mut reservation),
        Err(AdmittedStorageError::ScopeIdentity)
    ));
    assert_eq!(reservation.remaining_bytes(), layout.size());
    assert_eq!(source.reserved_bytes(), layout.size());
    assert_eq!(foreign.reserved_bytes(), 0);
    let sibling = reservation
        .try_split(std::alloc::Layout::new::<u8>())
        .unwrap();
    assert!(matches!(
        original_refund_scope(&source, &mut reservation),
        Err(AdmittedStorageError::PolicyDemand { expected_bytes, remaining_bytes })
            if expected_bytes == layout.size() && remaining_bytes == layout.size() - 1
    ));
    assert_eq!(reservation.remaining_bytes(), layout.size() - 1);
    drop((sibling, reservation));
    assert_eq!(source.reserved_bytes(), 0);
    let mut reservation = source.try_reserve(layout).unwrap();
    source.set_limit_bytes(0);
    let scope = original_refund_scope(&source, &mut reservation).unwrap();
    assert!(scope.belongs_to(&source));
    assert!(!scope.belongs_to(&foreign));
    assert_eq!(reservation.remaining_bytes(), 0);
    assert_eq!(source.reserved_bytes(), layout.size());
    drop(scope);
    assert_eq!(source.reserved_bytes(), 0);
}
