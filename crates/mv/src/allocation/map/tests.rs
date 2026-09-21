//! Physical original-map and original-pool retention controls.

use super::super::tests::without_allocations;
use super::*;

struct Scalar;
impl CopyPolicy<u64, u64> for Scalar {
    fn plan_key(_: &u64, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &u64, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn copy_key(key: &u64, _: &mut AllocationReservation) -> u64 {
        *key
    }
    fn copy_value(value: &u64, _: &mut AllocationReservation) -> u64 {
        *value
    }
}

type Map = BudgetMap<u64, u64, Scalar>;

// Seed actual nonempty original cursors through the existing closed insertion.
// This test helper is not a public mutation/import path or State qualification.
fn insert(writer: &mut BudgetWriter<'_, u64, u64, Scalar>, key: u64, value: u64) {
    let budget = writer.budget;
    assert!(
        writer
            .inner
            .try_insert_admitted(key, value, |demand| {
                budget.try_reserve_bytes(demand.bytes()).map(Provider::new)
            })
            .unwrap_or_else(|_| panic!("original insertion funding"))
            .is_none()
    );
}

#[test]
fn original_pool_equality_follows_retained_control_not_handle_address_or_limit() {
    let pool = AllocationBudget::new(4096);
    let cloned_handle = pool.clone();
    let foreign = AllocationBudget::new(4096);
    without_allocations(|| {
        assert!(pool.same_pool(&cloned_handle));
        assert!(!pool.same_pool(&foreign));
    });
    drop(pool);
    assert!(!cloned_handle.same_pool(&foreign));
}

#[test]
fn construction_and_writer_refusal_use_only_the_original_finite_pool() {
    let zero = AllocationBudget::new(0);
    assert!(matches!(
        without_allocations(|| Map::try_new_with_node_custody(&zero)),
        Err(AllocationRefusal::ExceedsLimit { .. })
    ));
    assert_eq!(zero.reserved_bytes(), 0);
    let pool = AllocationBudget::new(1 << 20);
    let map =
        pool.with_deferred_refund_notifications(|| Map::try_new_with_node_custody(&pool).unwrap());
    let initial = pool.reserved_bytes();
    assert!(initial > 0);
    let blocker = pool
        .try_reserve_bytes(pool.limit_bytes() - initial)
        .unwrap();
    let held = pool.reserved_bytes();
    assert!(matches!(
        without_allocations(|| map.try_write()),
        Err(InsertAdmissionError::Refused(
            AllocationRefusal::Capacity { .. }
        ))
    ));
    assert_eq!(pool.reserved_bytes(), held);
    pool.with_deferred_refund_notifications(|| {
        drop(blocker);
        let writer = map
            .try_write()
            .unwrap_or_else(|_| panic!("original writer"));
        assert_eq!(writer.get(&7), None);
        without_allocations(|| drop(writer));
    });
    assert_eq!(pool.reserved_bytes(), initial);
    without_allocations(|| pool.with_deferred_refund_notifications(|| drop(map)));
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn foreign_pool_refuses_before_busy_map_acquisition_and_returns_original_successor() {
    let source_pool = AllocationBudget::new(1 << 20);
    let target_pool = AllocationBudget::new(1 << 20);
    let source = source_pool.with_deferred_refund_notifications(|| {
        Map::try_new_with_node_custody(&source_pool).unwrap()
    });
    let target = target_pool.with_deferred_refund_notifications(|| {
        Map::try_new_with_node_custody(&target_pool).unwrap()
    });
    let owned = source_pool.with_deferred_refund_notifications(|| {
        let mut writer = source
            .try_write()
            .unwrap_or_else(|_| panic!("source writer"));
        insert(&mut writer, 7, 21);
        writer.detach()
    });
    let original_source_bytes = source_pool.reserved_bytes();
    let owned = target_pool.with_deferred_refund_notifications(|| {
        let busy = target
            .try_write()
            .unwrap_or_else(|_| panic!("target writer"));
        let target_bytes = target_pool.reserved_bytes();
        let (owned, error) =
            without_allocations(|| target.try_write_owned(owned).err().expect("foreign pool"));
        assert!(matches!(error, OwnerRefusal::ForeignPool));
        assert_eq!(owned.get(&7), Some(&21));
        assert_eq!(source_pool.reserved_bytes(), original_source_bytes);
        assert_eq!(target_pool.reserved_bytes(), target_bytes);
        without_allocations(|| drop(busy));
        owned
    });
    source_pool.with_deferred_refund_notifications(|| {
        let writer = without_allocations(|| {
            source
                .try_write_owned(owned)
                .unwrap_or_else(|_| panic!("same source"))
        });
        assert_eq!(writer.get(&7), Some(&21));
        without_allocations(|| writer.commit());
    });
    source_pool.with_deferred_refund_notifications(|| drop(source));
    target_pool.with_deferred_refund_notifications(|| drop(target));
    assert_eq!(source_pool.reserved_bytes(), 0);
    assert_eq!(target_pool.reserved_bytes(), 0);
}

#[test]
fn equal_pool_does_not_rebind_a_successor_to_another_physical_map() {
    let pool = AllocationBudget::new(1 << 20);
    pool.with_deferred_refund_notifications(|| {
        let source = Map::try_new_with_node_custody(&pool).unwrap();
        let foreign = Map::try_new_with_node_custody(&pool.clone()).unwrap();
        let mut writer = source
            .try_write()
            .unwrap_or_else(|_| panic!("original writer"));
        insert(&mut writer, 7, 21);
        let owned = without_allocations(|| writer.detach());
        let baseline = pool.reserved_bytes();
        let (owned, error) = without_allocations(|| {
            foreign
                .try_write_owned(owned)
                .err()
                .expect("wrong original map")
        });
        assert!(matches!(error, OwnerRefusal::Map(OwnedWriteError::Changed)));
        assert_eq!(pool.reserved_bytes(), baseline);
        assert_eq!(owned.get(&7), Some(&21));
        let writer = without_allocations(|| {
            source
                .try_write_owned(owned)
                .unwrap_or_else(|_| panic!("same map"))
        });
        assert_eq!(writer.get(&7), Some(&21));
        without_allocations(|| writer.commit());
    });
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn nested_original_checkpoints_preserve_pool_preimages_and_parent_rollback() {
    let pool = AllocationBudget::new(1 << 20);
    pool.with_deferred_refund_notifications(|| {
        let map = Map::try_new_with_node_custody(&pool).unwrap();
        let mut writer = map
            .try_write()
            .unwrap_or_else(|_| panic!("original writer"));
        insert(&mut writer, 7, 21);
        let mut first = without_allocations(|| writer.checkpoint().unwrap());
        assert!(first.budget.same_pool(&pool));
        assert_eq!(first.get(&7), Some(&21));
        let original_pool = first.budget;
        assert_eq!(
            first
                .inner
                .try_insert_admitted(7, 99, |demand| original_pool
                    .try_reserve_bytes(demand.bytes())
                    .map(Provider::new))
                .unwrap_or_else(|_| panic!("original child funding")),
            Some(21)
        );
        assert_eq!(first.get_before(&7), Some(&21));
        {
            let second = without_allocations(|| first.checkpoint().unwrap());
            assert!(second.budget.same_pool(&pool));
            assert_eq!(second.get(&7), Some(&99));
            assert_eq!(second.get_before(&7), Some(&99));
            without_allocations(|| second.apply());
        }
        without_allocations(|| drop(first));
        assert_eq!(writer.get(&7), Some(&21));
        without_allocations(|| writer.commit());
        let owner = map
            .try_write()
            .unwrap_or_else(|_| panic!("published original"))
            .detach();
        assert_eq!(owner.get(&7), Some(&21));
        without_allocations(|| drop(map));
        assert_eq!(owner.get(&7), Some(&21));
        assert!(pool.reserved_bytes() > 0);
        without_allocations(|| drop(owner));
    });
    assert_eq!(pool.reserved_bytes(), 0);
}
