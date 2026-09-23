//! Exact immutable reader custody without reopening a current publication cut.
use super::*;
use crate::internals::bptree::node::allocation_tests::without_allocations;
use std::{
    future::Future,
    task::{Context, Waker},
};

#[test]
fn retained_reader_clone_keeps_old_generation_without_allocation() {
    let map = BptreeMap::<usize, usize>::new();
    let mut writer = map.write();
    writer.insert(1, 10);
    writer.commit();
    let original = map.read();
    let retained = original.predecessor().retain();
    let cloned = without_allocations(|| original.clone());
    let mut writer = map.write();
    writer.insert(1, 20);
    writer.insert(2, 30);
    writer.commit();
    let reopened = without_allocations(|| map.read_predecessor(&retained).unwrap());
    for reader in [&original, &cloned, &reopened] {
        assert_eq!(reader.get(&1), Some(&10));
        assert_eq!(reader.get(&2), None);
        assert!(retained.matches(&reader.predecessor()));
    }
    assert_eq!(map.read().get(&1), Some(&20));
}

#[test]
fn equal_foreign_family_cannot_reopen_retained_reader() {
    let first = BptreeMap::<usize, usize>::new();
    let second = BptreeMap::<usize, usize>::new();
    let original = first.read().predecessor().retain();
    assert!(without_allocations(|| second.read_predecessor(&original)).is_none());
    assert!(without_allocations(|| first.read_predecessor(&original)).is_some());
}

#[test]
fn pinned_reader_reopens_while_actual_active_lock_is_owned_without_a_notice() {
    let map = BptreeMap::<usize, usize>::new();
    let original = map.read().predecessor().retain();
    let prepared = map.write().prepare_commit();
    let wait = map.observe_reader_release();
    let mut future = std::pin::pin!(wait.wait_for_release());
    let mut context = Context::from_waker(Waker::noop());
    assert!(future.as_mut().poll(&mut context).is_pending());
    let reader = without_allocations(|| map.read_predecessor(&original).unwrap());
    let copy = without_allocations(|| reader.clone());
    assert!(reader.is_empty() && copy.is_empty());
    drop((reader, copy));
    assert!(
        future.as_mut().poll(&mut context).is_pending(),
        "exact pinned reads acquire/release no active-reader mutex"
    );
    drop(prepared);
    assert!(future.as_mut().poll(&mut context).is_ready());
}

#[test]
fn equal_value_aba_preserves_old_reader_identity() {
    let map = BptreeMap::<usize, usize>::new();
    let mut writer = map.write();
    writer.insert(1, 10);
    writer.commit();
    let original = map.read().predecessor().retain();
    let mut writer = map.write();
    writer.insert(1, 20);
    writer.commit();
    let mut writer = map.write();
    writer.insert(1, 10);
    writer.commit();
    let latest = map.read();
    assert!(!original.matches(&latest.predecessor()));
    let old = map.read_predecessor(&original).unwrap();
    assert_eq!(old.get(&1), latest.get(&1));
    assert!(original.matches(&old.predecessor()));
}

struct ScalarPolicy;
impl NodeFunding for ScalarPolicy {
    type Charge = Untracked;
    fn take_node_charge(&mut self, _: std::alloc::Layout) -> Untracked {
        Untracked
    }
}
impl NodeCloning<usize, usize> for ScalarPolicy {
    fn clone_key(&mut self, key: &usize) -> usize {
        *key
    }
    fn clone_value(&mut self, value: &usize) -> usize {
        *value
    }
}
impl ClonePlanning<usize, usize> for ScalarPolicy {
    fn plan_key(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}

#[test]
fn empty_successor_floor_uses_same_original_native_inventory_as_insertion() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let mut writer_floor = None;
    assert!(matches!(
        without_allocations(
            || map.try_write_admitted_with_footprint(|existing, additional| {
                writer_floor = Some((existing.bytes(), additional.bytes()));
                Err::<ScalarPolicy, _>(())
            })
        ),
        Err(MapAdmissionError::Refused(()))
    ));
    let (existing, additional) = writer_floor.unwrap();
    let mut insertion_floor = None;
    assert!(matches!(
        without_allocations(
            || map.try_insert_admitted_with_footprint(1, 10, |floor, demand| {
                insertion_floor = Some((floor.bytes(), demand.bytes()));
                Err::<ScalarPolicy, _>(())
            })
        ),
        Err((_, MapAdmissionError::Refused(())))
    ));
    assert_eq!(existing, insertion_floor.unwrap().0);
    assert!(additional > 0 && additional < insertion_floor.unwrap().1);
    assert!(map.read().is_empty());
    let work = map
        .try_write_admitted_with_footprint(|floor, demand| {
            assert_eq!((floor.bytes(), demand.bytes()), (existing, additional));
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap()
        .detach();
    assert_eq!(
        work.required_allocation_floor().unwrap().bytes(),
        existing + additional
    );
}

#[test]
fn busy_writer_footprint_never_calls_provider_or_constructs_a_cursor() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let held = map.try_acquire_writer().unwrap();
    let mut called = false;
    assert!(matches!(
        without_allocations(|| map.try_write_admitted_with_footprint(|_, _| {
            called = true;
            Ok::<_, ()>(ScalarPolicy)
        })),
        Err(MapAdmissionError::Busy)
    ));
    assert!(!called);
    drop(held);
    drop(
        map.try_write_admitted_with_footprint(|_, _| Ok::<_, ()>(ScalarPolicy))
            .unwrap(),
    );
}
