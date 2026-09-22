//! Actual prepaid storage owners pass through the World mode adapter.

use super::*;
use concread::bptree::{AllocationDemand, NodeCloning, NodeFunding, PlanningError};
use mv::{
    allocation::{AllocationBudget, AllocationCharge, AllocationReservation},
    storage::AdmittedStoragePolicy,
};
use std::alloc::Layout;

struct Policy(AllocationReservation);
impl NodeFunding for Policy {
    type Charge = AllocationCharge;
    fn take_node_charge(&mut self, layout: Layout) -> Self::Charge {
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
type Mode = Prepaid<Policy>;
type Target = Storage<u64, u64, Mode>;
type Journal = mv::storage::Detached<u64, u64, (), Mode>;

fn fixture(budget: &AllocationBudget) -> (Target, Journal) {
    let target = Target::try_new_admitted(budget.clone()).unwrap();
    target
        .try_with_admitted_block(|block| {
            block.try_insert_admitted(7, 70).unwrap();
            Ok::<_, ()>(())
        })
        .unwrap();
    let journal = target
        .try_capture_admitted_block(BlockMode::Ordinary, |block| {
            block.try_insert_admitted(7, 71).unwrap();
            block.try_insert_admitted(9, 90).unwrap();
            Ok::<_, ()>(())
        })
        .unwrap();
    (target, journal)
}
fn image(journal: &Journal) -> Vec<(u64, Option<u64>, Option<u64>)> {
    journal
        .touched_entries()
        .map(|entry| (*entry.key, entry.before.copied(), entry.after.copied()))
        .collect()
}

#[test]
fn prepaid_world_storage_adapter_refuses_missing_and_foreign_scope_before_writers() {
    let budget = AllocationBudget::new(1024 * 1024);
    let foreign = AllocationBudget::new(1024 * 1024);
    let (target, mut journal) = fixture(&budget);
    let expected = image(&journal);
    let retained_bytes = budget.reserved_bytes();
    let mut slot = <Mode as WorldStorageMode<u64, u64>>::publication_slot(journal, &target, None);
    assert!(matches!(
        <Mode as WorldStorageMode<u64, u64>>::try_prepare(&mut slot),
        Err(PublicationPreparationError::Admission(
            AdmittedStorageError::ScopeIdentity
        ))
    ));
    journal = <Mode as WorldStorageMode<u64, u64>>::recover_original(&mut slot);
    drop(slot);
    assert_eq!(image(&journal), expected);
    assert_eq!(budget.reserved_bytes(), retained_bytes);
    foreign.with_deferred_refund_notifications(|scope| {
        let mut slot =
            <Mode as WorldStorageMode<u64, u64>>::publication_slot(journal, &target, Some(scope));
        assert!(matches!(
            <Mode as WorldStorageMode<u64, u64>>::try_prepare(&mut slot),
            Err(PublicationPreparationError::Admission(
                AdmittedStorageError::ScopeIdentity
            ))
        ));
        let journal = <Mode as WorldStorageMode<u64, u64>>::recover_original(&mut slot);
        // Real original-scope reacquisition succeeds while the refused shell is still alive.
        budget.with_deferred_refund_notifications(|original_scope| {
            let prepared = journal
                .try_prepare_admitted(original_scope, &target)
                .unwrap_or_else(|(_, error, _)| {
                    panic!("scope refusal acquired writers: {error:?}")
                });
            let (journal, cleanup) = prepared.abort();
            assert_eq!(image(&journal), expected);
            drop(cleanup);
            drop(journal);
        });
        drop(slot);
    });
    assert_eq!(target.view().get(&7), Some(&70));
    drop(target);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(foreign.reserved_bytes(), 0);
}

#[test]
fn prepaid_world_storage_adapter_preserves_original_pair_through_abort_and_publish() {
    let budget = AllocationBudget::new(1024 * 1024);
    let (target, journal) = fixture(&budget);
    let expected = image(&journal);
    let reader = target.view();
    let retained_bytes = budget.reserved_bytes();
    let journal = budget.with_deferred_refund_notifications(|scope| {
        let mut slot =
            <Mode as WorldStorageMode<u64, u64>>::publication_slot(journal, &target, Some(scope));
        <Mode as WorldStorageMode<u64, u64>>::try_prepare(&mut slot).unwrap();
        let prepared = <Mode as WorldStorageMode<u64, u64>>::into_prepared(slot);
        let (journal, cleanup) = <Mode as WorldStorageMode<u64, u64>>::abort(prepared);
        assert_eq!(image(&journal), expected);
        drop(cleanup);
        journal
    });
    assert_eq!(
        budget.reserved_bytes(),
        retained_bytes,
        "abort does not reconstruct the pair"
    );
    budget.with_deferred_refund_notifications(|scope| {
        let mut slot =
            <Mode as WorldStorageMode<u64, u64>>::publication_slot(journal, &target, Some(scope));
        <Mode as WorldStorageMode<u64, u64>>::try_prepare(&mut slot).unwrap();
        let prepared = <Mode as WorldStorageMode<u64, u64>>::into_prepared(slot);
        let published = <Mode as WorldStorageMode<u64, u64>>::publish(prepared);
        assert_eq!(reader.get(&7), Some(&70), "retained original reader");
        assert_eq!(target.view().get(&7), Some(&71));
        assert_eq!(target.view().get(&9), Some(&90));
        drop(published);
    });
    let snapshot = target.snapshot();
    assert_eq!(snapshot.revert_map().get(&7), Some(&Some(70)));
    assert_eq!(snapshot.revert_map().get(&9), Some(&None));
    drop(snapshot);
    drop(reader);
    drop(target);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn prepaid_world_storage_adapter_busy_retry_keeps_exact_original_values() {
    let budget = AllocationBudget::new(1024 * 1024);
    let (target, journal) = fixture(&budget);
    let sibling = target
        .try_capture_admitted_block(BlockMode::Ordinary, |block| {
            block.try_insert_admitted(7, 72).unwrap();
            Ok::<_, ()>(())
        })
        .unwrap();
    let expected = image(&journal);
    let journal = budget.with_deferred_refund_notifications(|scope| {
        let held = sibling
            .try_prepare_admitted(scope, &target)
            .unwrap_or_else(|(_, error, _)| panic!("held original: {error:?}"));
        let mut slot =
            <Mode as WorldStorageMode<u64, u64>>::publication_slot(journal, &target, Some(scope));
        assert!(matches!(
            <Mode as WorldStorageMode<u64, u64>>::try_prepare(&mut slot),
            Err(PublicationPreparationError::Busy(_))
        ));
        let journal = <Mode as WorldStorageMode<u64, u64>>::recover_original(&mut slot);
        assert_eq!(image(&journal), expected);
        let (sibling, cleanup) = held.abort();
        drop(cleanup);
        drop(slot);
        drop(sibling);
        journal
    });
    budget.with_deferred_refund_notifications(|scope| {
        let mut slot =
            <Mode as WorldStorageMode<u64, u64>>::publication_slot(journal, &target, Some(scope));
        <Mode as WorldStorageMode<u64, u64>>::try_prepare(&mut slot).unwrap();
        // Exercise the ordinary recovery transition after complete preparation.
        let journal = <Mode as WorldStorageMode<u64, u64>>::recover_original(&mut slot);
        assert_eq!(image(&journal), expected);
        <Mode as WorldStorageMode<u64, u64>>::release_writers(&mut slot);
        drop(slot);
        drop(journal);
    });
    assert_eq!(target.view().get(&7), Some(&70));
    drop(target);
    assert_eq!(budget.reserved_bytes(), 0);
}
