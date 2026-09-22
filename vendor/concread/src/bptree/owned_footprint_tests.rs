//! Exact allocator custody of fixed-size owned batches and their retained bases.

use super::*;
use crate::internals::bptree::node::allocation_tests::{
    all_refunded, prepaid, record, without_allocations, Charge, Prepaid as Records,
};
use std::{cell::RefCell, rc::Rc};

thread_local! {
    static PANIC_COPY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

struct Policy {
    records: Rc<RefCell<Records>>,
    remaining: usize,
}

impl NodeFunding for Policy {
    type Charge = Charge;

    fn take_node_charge(&mut self, layout: Layout) -> Charge {
        self.remaining = self
            .remaining
            .checked_sub(layout.size())
            .expect("admitted bytes");
        self.records
            .borrow_mut()
            .take_batch_allocation_charge(layout)
    }
}

impl NodeCloning<usize, usize> for Policy {
    fn clone_key(&mut self, key: &usize) -> usize {
        *key
    }

    fn clone_value(&mut self, value: &usize) -> usize {
        assert!(!PANIC_COPY.with(std::cell::Cell::get), "copy failed");
        *value
    }
}

impl ClonePlanning<usize, usize> for Policy {
    fn plan_key(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }

    fn plan_value(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}

type Map = BptreeMap<usize, usize, Prepaid<Policy>>;
type Owned = BptreeMapOwned<usize, usize, Prepaid<Policy>>;

fn admit(records: &Rc<RefCell<Records>>, demand: AllocationDemand) -> Result<Policy, ()> {
    Ok(Policy {
        records: records.clone(),
        remaining: demand.bytes(),
    })
}

fn fixture() -> (Rc<RefCell<Records>>, Map) {
    let records = Rc::new(RefCell::new(prepaid()));
    let map = Map::try_new_with_node_custody(|demand| admit(&records, demand)).unwrap();
    (records, map)
}

fn owned(map: &Map, records: &Rc<RefCell<Records>>) -> Owned {
    map.try_write_admitted(|demand| admit(records, demand))
        .unwrap()
        .detach()
}

// Independent oracle: each charge identifies an actual allocator call and may
// refund only after that exact pointer/layout has been physically freed.
fn live(records: &Rc<RefCell<Records>>) -> AllocationDemand {
    let mut demand = AllocationDemand::new();
    for id in 0..records.as_ref().borrow().next {
        let observed = record(id);
        if !observed.freed {
            assert!(!observed.refunded);
            demand.add_layout(observed.layout).unwrap();
        }
    }
    demand
}

fn floor(owner: &Owned) -> AllocationDemand {
    without_allocations(|| owner.required_allocation_floor().unwrap())
}

fn exact(owner: &Owned, records: &Rc<RefCell<Records>>) -> AllocationDemand {
    let actual = floor(owner);
    assert_eq!(actual, live(records));
    actual
}

#[test]
fn owned_floor_matches_actual_growth_overwrite_removal_and_retained_base() {
    let (records, map) = fixture();
    let mut owner = owned(&map, &records);
    assert_eq!(exact(&owner, &records).allocations(), 5);
    for key in 0..48 {
        owner
            .try_insert_admitted(key, key * 3, |d| admit(&records, d))
            .unwrap();
        exact(&owner, &records);
    }
    let grown = exact(&owner, &records);
    assert!(grown.allocations() > 8, "actual leaf and branch growth");
    for key in 0..48 {
        assert_eq!(
            owner
                .try_insert_admitted(key, key * 5, |d| admit(&records, d))
                .unwrap(),
            Some(key * 3)
        );
    }
    assert_eq!(
        exact(&owner, &records),
        grown,
        "private overwrite needs no node clone"
    );
    for key in 0..48 {
        assert_eq!(
            owner
                .try_remove_admitted(&key, |d| admit(&records, d))
                .unwrap(),
            Some(key * 5)
        );
        exact(&owner, &records);
    }
    assert!(owner.is_empty());
    assert!(owner.inner.as_ref().admitted_tracking()[1].0 > 1);
    assert!(
        floor(&owner).bytes() >= grown.bytes(),
        "retired private nodes are still allocated"
    );
    assert!(
        map.read().is_empty(),
        "original committed base remains unchanged"
    );
    drop(map);
    exact(&owner, &records);
    drop(owner);
    assert_eq!(live(&records), AllocationDemand::new());
    all_refunded(&records.as_ref().borrow());
}

#[test]
fn owned_floor_preserves_snapshot_reacquisition_and_checkpoint_apply_abort_custody() {
    let (records, map) = fixture();
    let mut owner = owned(&map, &records);
    for key in 0..16 {
        owner
            .try_insert_admitted(key, key, |d| admit(&records, d))
            .unwrap();
    }
    map.try_write_owned(owner)
        .unwrap_or_else(|_| panic!("original seed"))
        .commit();
    let mut owner = owned(&map, &records);
    owner
        .try_insert_admitted(100, 700, |d| admit(&records, d))
        .unwrap();
    let before = exact(&owner, &records);
    let cursor = std::ptr::from_ref(owner.inner.as_ref());
    let original = owner.predecessor().retain();
    {
        let snapshot = owner.to_snapshot();
        assert_eq!(snapshot.get(&100), Some(&700));
        assert_eq!(floor(&owner), before);
    }
    let mut writer = without_allocations(|| {
        map.try_write_owned(owner)
            .unwrap_or_else(|_| panic!("same base"))
    });
    let checkpoint_start = records.as_ref().borrow().next;
    {
        let mut outer = without_allocations(|| writer.checkpoint().unwrap());
        for key in 16..32 {
            outer
                .try_insert_admitted(key, key, |d| admit(&records, d))
                .unwrap();
        }
        let mut inner = outer.checkpoint().unwrap();
        inner
            .try_remove_admitted(&0, |d| admit(&records, d))
            .unwrap();
        inner.apply();
        // Applying the child does not release the parent's original rollback.
    }
    for id in checkpoint_start..records.as_ref().borrow().next {
        let record = record(id);
        assert!(
            record.freed && record.refunded,
            "aborted suffix really freed"
        );
    }
    let owner = without_allocations(|| writer.detach());
    assert_eq!(std::ptr::from_ref(owner.inner.as_ref()), cursor);
    assert!(original.matches(&owner.predecessor()));
    assert_eq!(exact(&owner, &records), before);
    assert_eq!(owner.get(&0), Some(&0));
    assert_eq!(owner.get(&16), None);
    let mut writer = map
        .try_write_owned(owner)
        .unwrap_or_else(|_| panic!("retry after rollback"));
    {
        let mut checkpoint = writer.checkpoint().unwrap();
        checkpoint
            .try_insert_admitted(101, 701, |d| admit(&records, d))
            .unwrap();
        checkpoint.apply();
    }
    let owner = writer.detach();
    assert!(exact(&owner, &records).bytes() > before.bytes());
    let prepared = map
        .try_write_owned(owner)
        .unwrap_or_else(|_| panic!("same publication base"))
        .prepare_commit();
    let owner = without_allocations(|| prepared.abort().detach());
    assert!(original.matches(&owner.predecessor()));
    exact(&owner, &records);
    map.try_write_owned(owner)
        .unwrap_or_else(|_| panic!("retry prepared abort"))
        .commit();
    assert_eq!(map.read().get(&101), Some(&701));
    drop(original);
    drop(map);
    all_refunded(&records.as_ref().borrow());
}

#[test]
fn owned_floor_distinguishes_irreducible_reservation_from_refundable_reader_pressure() {
    let (records, map) = fixture();
    let old = map.read();
    let mut batch = owned(&map, &records);
    for key in 0..32 {
        batch
            .try_insert_admitted(key, key, |d| admit(&records, d))
            .unwrap();
    }
    map.try_write_owned(batch)
        .unwrap_or_else(|_| panic!("publish batch"))
        .commit();
    let mut owner = owned(&map, &records);
    let before = floor(&owner);
    let mut reservation = before;
    let next = without_allocations(|| owner.insertion_demand(&100).unwrap());
    reservation.add_demand(next).unwrap();
    let limit = reservation.bytes();
    assert!(live(&records).bytes() > before.bytes());
    let cursor = std::ptr::from_ref(owner.inner.as_ref());
    let refused = without_allocations(|| {
        owner.try_insert_admitted(100, 700, |d| {
            assert_eq!(d, next);
            assert!(live(&records).bytes().checked_add(d.bytes()).unwrap() > limit);
            Err::<Policy, _>("reader pressure")
        })
    });
    let Err(((key, value), MapAdmissionError::Refused("reader pressure"))) = refused else {
        panic!("original input must survive refusal");
    };
    assert_eq!((key, value), (100, 700));
    assert_eq!(std::ptr::from_ref(owner.inner.as_ref()), cursor);
    assert_eq!(floor(&owner), before);
    assert!(owner.get(&key).is_none());
    drop(old);
    assert_eq!(
        exact(&owner, &records),
        before,
        "only external old-reader pressure refunded"
    );
    let refused = without_allocations(|| {
        owner.try_insert_admitted(key, value, |d| {
            let mut needed = before;
            needed.add_demand(d).unwrap();
            assert!(needed.bytes() > limit - 1);
            Err::<Policy, _>("irreducible")
        })
    });
    let Err(((key, value), MapAdmissionError::Refused("irreducible"))) = refused else {
        panic!("even no external readers cannot satisfy the smaller limit");
    };
    owner
        .try_insert_admitted(key, value, |d| {
            assert!(live(&records).bytes().checked_add(d.bytes()).unwrap() <= limit);
            admit(&records, d)
        })
        .unwrap();
    exact(&owner, &records);
    map.try_write_owned(owner)
        .unwrap_or_else(|_| panic!("same refused input and base"))
        .commit();
    assert_eq!(map.read().get(&100), Some(&700));
    drop(map);
    all_refunded(&records.as_ref().borrow());
}

#[test]
fn owned_floor_excludes_stale_successor_chain_and_does_not_authorize_reacquisition() {
    let (records, map) = fixture();
    let mut owner = owned(&map, &records);
    owner
        .try_insert_admitted(1, 11, |d| admit(&records, d))
        .unwrap();
    let before = exact(&owner, &records);
    let cursor = std::ptr::from_ref(owner.inner.as_ref());
    for key in 2..5 {
        let mut writer = map.try_write_admitted(|d| admit(&records, d)).unwrap();
        writer
            .try_insert_admitted(key, key * 11, |d| admit(&records, d))
            .unwrap();
        writer.commit();
    }
    assert_eq!(floor(&owner), before);
    assert!(
        live(&records).bytes() > before.bytes(),
        "stale base pins additional successor custody"
    );
    let (owner, error) =
        without_allocations(|| map.try_write_owned(owner).err().expect("stale original"));
    assert_eq!(error, OwnedWriteError::Changed);
    assert_eq!(std::ptr::from_ref(owner.inner.as_ref()), cursor);
    assert_eq!(floor(&owner), before);
    assert_eq!(owner.to_snapshot().get(&1), Some(&11));
    assert_eq!(owner.to_snapshot().get(&2), None);
    drop(map);
    assert!(live(&records).bytes() > floor(&owner).bytes());
    drop(owner);
    all_refunded(&records.as_ref().borrow());
}

#[test]
fn owned_floor_rejects_a_caught_copy_panic_without_new_authority() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let (records, map) = fixture();
    let mut seed = owned(&map, &records);
    seed.try_insert_admitted(1, 11, |d| admit(&records, d))
        .unwrap();
    map.try_write_owned(seed)
        .unwrap_or_else(|_| panic!("seed publication"))
        .commit();
    let mut owner = owned(&map, &records);
    PANIC_COPY.with(|value| value.set(true));
    let failed = catch_unwind(AssertUnwindSafe(|| {
        owner
            .try_insert_admitted(2, 22, |d| admit(&records, d))
            .unwrap();
    }));
    PANIC_COPY.with(|value| value.set(false));
    assert!(failed.is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| owner.required_allocation_floor())).is_err());
    assert_eq!(map.read().get(&1), Some(&11));
    assert_eq!(map.read().get(&2), None);
    assert!(
        !map.is_poisoned(),
        "private edits acquire no physical writer"
    );
    drop(owner);
    drop(map);
    all_refunded(&records.as_ref().borrow());
}
