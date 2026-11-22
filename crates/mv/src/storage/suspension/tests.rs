//! Actual map roots, native locks and prepaid custody through execution suspension.

use super::*;
use crate::allocation_test_support::without_allocations;
use concread::bptree::{AllocationDemand, ClonePlanning, NodeFunding, PlanningError, Prepaid};
use iroha_allocation::{AllocationBudget, AllocationCharge, AllocationReservation};
use std::{
    alloc::Layout,
    panic::{AssertUnwindSafe, catch_unwind},
};

fn suspend<K: Key, V: Value, M: StorageMode<K, V>>(
    block: Block<'_, K, V, M>,
) -> SuspendedBlock<'_, K, V, M> {
    let mut slot = block.suspension_slot();
    slot.suspend();
    let (original, cleanup) = slot.into_suspended();
    drop(cleanup);
    original
}

#[test]
fn original_map_roots_undo_touches_and_execution_metadata_survive_both_modes() {
    for replace in [false, true] {
        let target = Storage::from_iter([(1_u64, 10_u64), (2, 20)]);
        let mut previous = target.block();
        previous.insert(1, 11);
        previous.commit();
        let mut block = if replace {
            target.block_and_revert()
        } else {
            target.block()
        };
        let expected_before = if replace { 10 } else { 11 };
        block.insert(1, 30);
        {
            let mut child = block.transaction();
            child.insert(1, 31);
            child.remove(99); // An absent-to-absent touch still belongs to this original journal.
            child.apply();
        }
        let current = std::ptr::from_ref(block.get(&1).unwrap());
        let undo = std::ptr::from_ref(block.get_before_block(&1).unwrap());
        let identity = block.publication_identity();
        let observations = block.write_observation();
        assert_eq!(observations, (true, 1));
        let original = without_allocations(|| suspend(block));
        assert_eq!(target.view().get(&1), Some(&11));
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
        assert_eq!(block.write_observation(), observations);
        assert_eq!(std::ptr::from_ref(block.get(&1).unwrap()), current);
        assert_eq!(
            std::ptr::from_ref(block.get_before_block(&1).unwrap()),
            undo
        );
        assert_eq!(block.get_before_block(&1), Some(&expected_before));
        assert_eq!(
            block
                .touched_entries()
                .map(|entry| *entry.key)
                .collect::<Vec<_>>(),
            [1, 99]
        );
        block.insert(1, 32);
        block.commit();
        drop(slot); // Only after the original pair releases may validation notices retire.
        assert_eq!(target.view().get(&1), Some(&32));
        assert_eq!(target.revert.read().get(&1), Some(&Some(expected_before)));
    }
}

#[test]
fn original_map_busy_refusal_keeps_exact_source_and_releases_partial_pair() {
    let target = Storage::from_iter([(1_u64, 10_u64)]);
    let mut block = target.block();
    block.insert(1, 20);
    let pointer = std::ptr::from_ref(block.get(&1).unwrap());
    let original = suspend(block);
    let held = target
        .blocks_released
        .poisoning_guard(target.blocks.write());
    let expected = target.blocks_released.observe();
    let mut slot = original.resumption_slot(&target);
    let error = without_allocations(|| slot.try_resume().unwrap_err());
    assert_eq!(error, PublicationPreparationError::Busy(expected));
    assert!(
        target.revert.try_acquire_writer().is_none(),
        "original partial undo remains installed until aggregate rollback"
    );
    let original = without_allocations(|| slot.recover_suspended());
    assert!(target.revert.try_acquire_writer().is_some());
    drop(slot);
    drop(held);
    let mut retry = original.resumption_slot(&target);
    without_allocations(|| retry.try_resume().unwrap());
    let block = retry.take_block();
    assert_eq!(std::ptr::from_ref(block.get(&1).unwrap()), pointer);
    assert_eq!(block.get_before_block(&1), Some(&10));
    block.commit();
    drop(retry);
    assert_eq!(target.view().get(&1), Some(&20));
}

#[test]
fn original_map_aggregate_partial_resumption_returns_all_same_journals() {
    let first = Storage::from_iter([(1_u64, 10_u64)]);
    let second = Storage::from_iter([(1_u64, 100_u64)]);
    let mut a = first.block();
    a.insert(1, 20);
    let mut b = second.block();
    b.insert(1, 200);
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
    let (a_original, b_original) =
        without_allocations(|| (a.recover_suspended(), b.recover_suspended()));
    drop((a, b));
    assert!(first.blocks.try_acquire_writer().is_some());
    assert!(first.revert.try_acquire_writer().is_some());
    assert!(second.revert.try_acquire_writer().is_some());
    assert_eq!(first.view().get(&1), Some(&10));
    assert_eq!(second.view().get(&1), Some(&100));
    drop(held);
    let mut a = a_original.resumption_slot(&first);
    let mut b = b_original.resumption_slot(&second);
    without_allocations(|| {
        a.try_resume().unwrap();
        b.try_resume().unwrap();
    });
    let a_block = a.take_block();
    let b_block = b.take_block();
    a_block.commit();
    b_block.commit();
    drop((a, b));
    assert_eq!(first.view().get(&1), Some(&20));
    assert_eq!(second.view().get(&1), Some(&200));
}

#[test]
fn original_map_refuses_foreign_owner_changed_parent_and_native_generation() {
    for mutation in 0..3 {
        let target = Storage::from_iter([(1_u64, 10_u64)]);
        let foreign = Storage::from_iter([(1_u64, 10_u64)]);
        let mut block = target.block();
        block.insert(1, 20);
        let original = suspend(block);
        if mutation == 1 {
            let mut published = target.block();
            published.insert(1, 12);
            published.commit();
        } else if mutation == 2 {
            // Even bypassing MV's identity in this private test cannot conceal a
            // changed original Concread generation from the real owned-map check.
            let mut raw = target.blocks.write();
            raw.insert(1, 13);
            raw.commit();
        }
        let mut slot = original.resumption_slot(if mutation == 0 { &foreign } else { &target });
        assert_eq!(
            without_allocations(|| slot.try_resume()),
            Err(PublicationPreparationError::Changed)
        );
        let original = without_allocations(|| slot.recover_suspended());
        drop(slot);
        if mutation == 0 {
            let mut retry = original.resumption_slot(&target);
            retry.try_resume().unwrap();
            let block = retry.take_block();
            assert_eq!(block.get(&1), Some(&20));
            drop(block);
            drop(retry);
        } else {
            drop(original);
        }
        assert_eq!(
            target.view().get(&1),
            Some(&match mutation {
                0 => 10,
                1 => 12,
                _ => 13,
            })
        );
    }
}

#[test]
fn original_map_partial_suspension_and_terminal_drop_release_without_allocation() {
    let first = Storage::from_iter([(1_u64, 10_u64)]);
    let second = Storage::from_iter([(1_u64, 100_u64)]);
    let mut a = first.block();
    a.insert(1, 20);
    let mut b = second.block();
    b.insert(1, 200);
    let mut a = a.suspension_slot();
    let mut b = b.suspension_slot();
    a.suspend();
    b.release_writers(); // An unexpected enclosing failure abandons, never revives.
    assert!(catch_unwind(AssertUnwindSafe(|| b.suspend())).is_err());
    without_allocations(|| {
        a.release_writers();
        b.release_writers();
        drop((a, b));
    });
    for target in [&first, &second] {
        assert!(target.blocks.try_acquire_writer().is_some());
        assert!(target.revert.try_acquire_writer().is_some());
    }
    assert_eq!(first.view().get(&1), Some(&10));
    assert_eq!(second.view().get(&1), Some(&100));
    let mut block = first.block();
    block.insert(1, 30);
    let original = suspend(block);
    let mut slot = original.resumption_slot(&first);
    slot.try_resume().unwrap();
    without_allocations(|| drop(slot));
    assert_eq!(first.view().get(&1), Some(&10));
    assert!(first.blocks.try_acquire_writer().is_some());
    assert!(first.revert.try_acquire_writer().is_some());
}

struct Policy(AllocationReservation);
impl NodeFunding for Policy {
    type Charge = AllocationCharge;
    fn take_node_charge(&mut self, layout: Layout) -> AllocationCharge {
        self.0.try_split(layout).unwrap()
    }
}
impl<V: Copy> NodeCloning<u64, V> for Policy {
    fn clone_key(&mut self, key: &u64) -> u64 {
        *key
    }
    fn clone_value(&mut self, value: &V) -> V {
        *value
    }
}
impl<V: Copy> ClonePlanning<u64, V> for Policy {
    fn plan_key(_: &u64, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &V, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}
impl AdmittedStoragePolicy for Policy {
    fn from_admission(reservation: AllocationReservation) -> Self {
        Self(reservation)
    }
    fn admission(&self) -> &AllocationReservation {
        &self.0
    }
}

#[test]
fn original_prepaid_scope_and_exhausted_pool_survive_suspension_without_new_admission() {
    let budget = AllocationBudget::new(1 << 20);
    let target = Storage::<u64, u64, Prepaid<Policy>>::try_new_admitted(budget.clone()).unwrap();
    let mut block = {
        let scope = budget.try_owned_refund_scope().unwrap();
        let mut acquisition = target.try_block_acquisition_owned(&scope).unwrap();
        acquisition.try_initialize(BlockMode::Ordinary).unwrap();
        acquisition.into_block()
    }; // Only the actual original Block now owns this refund scope.
    block.try_insert_admitted(1, 20).unwrap();
    let original_observation = (block.direct_write, block.applied_transactions);
    let pointer = std::ptr::from_ref(block.get(&1).unwrap());
    let original = without_allocations(|| suspend(block));
    let held = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
        .unwrap();
    let full = budget.reserved_bytes();
    let mut slot = original.resumption_slot(&target);
    without_allocations(|| slot.try_resume().unwrap());
    assert_eq!(budget.reserved_bytes(), full);
    let mut block = slot.take_block();
    assert_eq!(
        (block.direct_write, block.applied_transactions),
        original_observation
    );
    assert_eq!(std::ptr::from_ref(block.get(&1).unwrap()), pointer);
    // A direct insert may reuse a private leaf without allocating. A real child's
    // first touch must admit its ordered touch owner as well as the shared roots.
    let mut child = without_allocations(|| block.try_transaction_admitted().unwrap());
    let (input, error) = child.try_insert_admitted(2, 30).unwrap_err();
    assert_eq!(input, (2, 30));
    let AdmittedStorageError::Allocation(original) = error else {
        panic!("expected original finite-pool refusal");
    };
    let iroha_allocation::AllocationRefusal::Capacity { release, .. } = original else {
        panic!("expected retained original pool release");
    };
    assert!(budget.is_source_of(&release));
    assert_eq!(child.get(&2), None);
    assert_eq!(child.get(&1), Some(&20));
    drop(held);
    child.try_insert_admitted(2, 30).unwrap();
    child.apply();
    assert_eq!(block.get(&2), Some(&30));
    // Admitted insertion preserves the existing direct-write observation; only
    // applying this actual child advances its original applied-child count.
    assert_eq!(
        (block.direct_write, block.applied_transactions),
        (original_observation.0, original_observation.1 + 1)
    );
    drop(block);
    drop(slot);
    assert_eq!(target.view().get(&1), None);
    drop(target);
    assert_eq!(budget.reserved_bytes(), 0);
}
