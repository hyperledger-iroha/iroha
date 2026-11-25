//! Real EBR generations and finite allocation owners through mutable suspension.

use super::*;
use crate::allocation_test_support::without_allocations;
use iroha_allocation::{AllocationBudget, AllocationCharge};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn suspend<V: CellValue, C: Send + Sync + 'static>(block: Block<'_, V, C>) -> SuspendedBlock<V, C> {
    let mut slot = block.suspension_slot();
    slot.suspend();
    let (original, cleanup) = slot.into_suspended();
    drop(cleanup);
    original
}

#[test]
fn original_cell_generations_and_first_undo_survive_ordinary_and_replacement() {
    for replace in [false, true] {
        let target = Cell::new(10_u64);
        let mut previous = target.block();
        *previous.get_mut() = 11;
        previous.commit();
        let mut block = if replace {
            target.block_and_revert()
        } else {
            target.block()
        };
        let expected_before = if replace { 10 } else { 11 };
        *block.get_mut() = 20;
        let current = std::ptr::from_ref(block.get());
        let undo = std::ptr::from_ref(block.get_before_block());
        let identity = block.publication_identity();
        let original = without_allocations(|| suspend(block));
        assert_eq!(*target.view(), 11);
        let mut slot = original.resumption_slot(&target);
        without_allocations(|| slot.try_resume().unwrap());
        let mut block = slot.take_block();
        assert_eq!(block.publication_identity(), identity);
        assert_eq!(
            block.mode(),
            if replace {
                BlockMode::Replace
            } else {
                BlockMode::Ordinary
            }
        );
        assert!(block.is_dirty());
        assert_eq!(std::ptr::from_ref(block.get()), current);
        assert_eq!(std::ptr::from_ref(block.get_before_block()), undo);
        assert_eq!(*block.get_before_block(), expected_before);
        *block.get_mut() = 21;
        block.commit();
        drop(slot);
        assert_eq!(*target.view(), 21);
        assert_eq!(*target.predecessor_view(), Some(expected_before));
    }
}

#[test]
fn original_cell_untouched_replacement_stays_untouched_after_resumption() {
    let target = Cell::new(10_u64);
    let mut previous = target.block();
    *previous.get_mut() = 11;
    previous.commit();
    let original = suspend(target.block_and_revert());
    let mut slot = original.resumption_slot(&target);
    without_allocations(|| slot.try_resume().unwrap());
    let block = slot.take_block();
    assert_eq!(block.mode(), BlockMode::Replace);
    assert!(
        block.is_dirty(),
        "original replacement still has to publish its restored current generation"
    );
    assert_eq!(*block, 10);
    assert!(block.touched_value().is_none());
    block.commit();
    drop(slot);
    assert_eq!(*target.view(), 10);
    assert_eq!(*target.predecessor_view(), None);
}

#[test]
fn original_cell_busy_refusal_preserves_source_and_partial_undo_owner() {
    let target = Cell::new(10_u64);
    let mut block = target.block();
    *block.get_mut() = 20;
    let pointer = std::ptr::from_ref(block.get());
    let original = suspend(block);
    let held = target
        .blocks_released
        .poisoning_guard(target.blocks.write());
    let expected = target.blocks_released.observe();
    let mut slot = original.resumption_slot(&target);
    assert_eq!(
        without_allocations(|| slot.try_resume()),
        Err(PublicationPreparationError::Busy(expected))
    );
    assert!(target.revert.try_acquire_writer().is_none());
    let original = without_allocations(|| slot.recover_suspended());
    assert!(target.revert.try_acquire_writer().is_some());
    drop(slot);
    drop(held);
    let mut retry = original.resumption_slot(&target);
    without_allocations(|| retry.try_resume().unwrap());
    let block = retry.take_block();
    assert_eq!(std::ptr::from_ref(block.get()), pointer);
    block.commit();
    drop(retry);
    assert_eq!(*target.view(), 20);
    assert_eq!(*target.predecessor_view(), Some(10));
}

#[test]
fn original_cell_aggregate_partial_resumption_recovers_exact_originals() {
    let first = Cell::new(10_u64);
    let second = Cell::new(100_u64);
    let mut a = first.block();
    *a.get_mut() = 20;
    let mut b = second.block();
    *b.get_mut() = 200;
    let mut a = a.suspension_slot();
    let mut b = b.suspension_slot();
    without_allocations(|| {
        a.suspend();
        b.suspend();
    });
    let (a, a_cleanup) = a.into_suspended();
    let (b, b_cleanup) = b.into_suspended();
    drop((a_cleanup, b_cleanup));
    let held = second
        .blocks_released
        .poisoning_guard(second.blocks.write());
    let mut a = a.resumption_slot(&first);
    let mut b = b.resumption_slot(&second);
    without_allocations(|| {
        a.try_resume().unwrap();
        assert!(matches!(
            b.try_resume(),
            Err(PublicationPreparationError::Busy(_))
        ));
    });
    let (original_a, original_b) =
        without_allocations(|| (a.recover_suspended(), b.recover_suspended()));
    drop((a, b));
    assert!(first.blocks.try_acquire_writer().is_some());
    assert!(first.revert.try_acquire_writer().is_some());
    assert!(second.revert.try_acquire_writer().is_some());
    assert_eq!(*first.view(), 10);
    assert_eq!(*second.view(), 100);
    drop(held);
    let mut a = original_a.resumption_slot(&first);
    let mut b = original_b.resumption_slot(&second);
    without_allocations(|| {
        a.try_resume().unwrap();
        b.try_resume().unwrap();
    });
    let a_block = a.take_block();
    let b_block = b.take_block();
    a_block.commit();
    b_block.commit();
    drop((a, b));
    assert_eq!(*first.view(), 20);
    assert_eq!(*second.view(), 200);
}

#[test]
fn original_cell_refuses_foreign_owner_and_changed_published_predecessor() {
    for changed in [false, true] {
        let target = Cell::new(10_u64);
        let foreign = Cell::new(10_u64);
        let mut block = target.block();
        *block.get_mut() = 20;
        let original = suspend(block);
        if changed {
            let mut replacement = target.block();
            *replacement.get_mut() = 12;
            replacement.commit();
        }
        let mut slot = original.resumption_slot(if changed { &target } else { &foreign });
        assert_eq!(
            without_allocations(|| slot.try_resume()),
            Err(PublicationPreparationError::Changed)
        );
        let original = without_allocations(|| slot.recover_suspended());
        drop(slot);
        if !changed {
            let mut retry = original.resumption_slot(&target);
            retry.try_resume().unwrap();
            let block = retry.take_block();
            assert_eq!(*block, 20);
            drop(block);
            drop(retry);
        } else {
            drop(original);
        }
        assert_eq!(*target.view(), if changed { 12 } else { 10 });
    }
}

#[test]
fn original_cell_partial_suspension_and_drop_are_terminal_without_allocating() {
    let first = Cell::new(10_u64);
    let second = Cell::new(100_u64);
    let mut a = first.block();
    *a.get_mut() = 20;
    let mut b = second.block();
    *b.get_mut() = 200;
    let mut a = a.suspension_slot();
    let mut b = b.suspension_slot();
    a.suspend();
    b.release_writers();
    assert!(catch_unwind(AssertUnwindSafe(|| b.suspend())).is_err());
    without_allocations(|| {
        a.release_writers();
        b.release_writers();
        drop((a, b));
    });
    assert_eq!(*first.view(), 10);
    assert_eq!(*second.view(), 100);
    assert!(first.blocks.try_acquire_writer().is_some());
    assert!(first.revert.try_acquire_writer().is_some());
    assert!(second.blocks.try_acquire_writer().is_some());
    assert!(second.revert.try_acquire_writer().is_some());
    let mut block = first.block();
    *block.get_mut() = 30;
    let original = suspend(block);
    let mut slot = original.resumption_slot(&first);
    slot.try_resume().unwrap();
    without_allocations(|| drop(slot));
    assert_eq!(*first.view(), 10);
    assert!(first.blocks.try_acquire_writer().is_some());
    assert!(first.revert.try_acquire_writer().is_some());
}

#[test]
fn original_cell_real_generation_charges_are_retained_with_no_reacquisition_capacity() {
    type Funded = Cell<u64, AllocationCharge>;
    fn charges(budget: &AllocationBudget) -> CellAllocationCharges<AllocationCharge> {
        let [current, undo] = Funded::allocation_layouts();
        let mut reservation = budget.try_reserve_layouts([current, undo]).unwrap();
        CellAllocationCharges::new(
            reservation.try_split(current).unwrap(),
            reservation.try_split(undo).unwrap(),
        )
    }
    let pair: usize = Funded::allocation_layouts()
        .into_iter()
        .map(|layout| layout.size())
        .sum();
    let budget = AllocationBudget::new(2 * pair);
    let target = Funded::new_charged(10, charges(&budget));
    let mut block = target.block_charged(charges(&budget));
    *block.get_mut() = 20;
    let original = without_allocations(|| suspend(block));
    assert_eq!(budget.reserved_bytes(), 2 * pair);
    assert!(budget.try_reserve_bytes(1).is_err());
    let mut slot = original.resumption_slot(&target);
    without_allocations(|| slot.try_resume().unwrap());
    assert_eq!(budget.reserved_bytes(), 2 * pair);
    let mut block = slot.take_block();
    *block.get_mut() = 21;
    assert_eq!(*block.get_before_block(), 10);
    drop(block);
    drop(slot);
    assert_eq!(
        budget.reserved_bytes(),
        pair,
        "only unpublished original execution generations retired"
    );
    assert_eq!(*target.view(), 10);
    drop(target);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while budget.reserved_bytes() != 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "actual EBR generation charge remains"
        );
        crossbeam_epoch::pin().flush();
        std::thread::yield_now();
    }
}
