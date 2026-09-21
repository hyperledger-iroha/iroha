//! Checked planning and original input refusal before physical construction.

use super::*;
use crate::internals::bptree::node::allocation_tests::without_allocations;
use crate::internals::bptree::node::{TXID_MASK, TXID_SHF};

struct ScalarPolicy;
impl NodeFunding for ScalarPolicy {
    type Charge = Untracked;
    fn take_node_charge(&mut self, _: Layout) -> Untracked {
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
fn demand_overflow_preserves_the_original_sum_and_zero_layout_needs_no_allocation() {
    let mut demand = AllocationDemand::new();
    demand.add_layout(Layout::new::<()>()).unwrap();
    assert_eq!(demand.bytes(), 0);
    assert_eq!(demand.allocations(), 0);
    let large = Layout::from_size_align(isize::MAX as usize, 1).unwrap();
    demand.add_layout(large).unwrap();
    demand.add_layout(large).unwrap();
    let original = demand;
    assert_eq!(demand.add_layout(large), Err(PlanningError::Overflow));
    assert_eq!(demand, original);
    assert_eq!(demand.bytes(), usize::MAX - 1);
    assert_eq!(demand.allocations(), 2);
}

#[test]
fn empty_map_plan_includes_both_shells_fixed_buffers_and_full_root_growth_bound() {
    type Mode = Prepaid<ScalarPolicy>;
    let mut provider = Prepaid(Some(ScalarPolicy));
    let source = unsafe { SuperBlock::<usize, usize, Mode>::new_with_funding(&mut provider) };
    let shells = MapCell::<usize, usize, Mode>::writer_allocation_layouts();
    let plan = without_allocations(|| {
        plan_insert::<usize, usize, ScalarPolicy>(&source, &7, shells).unwrap()
    });
    assert_eq!((plan.first, plan.last), (3, 1));
    let expected = 2 * Layout::new::<CachePadded<Leaf<usize, usize>>>().size()
        + Layout::new::<CachePadded<Branch<usize, usize>>>().size()
        + 4 * Layout::new::<*mut Node<usize, usize>>().size()
        + shells.cursor.size()
        + shells.reader.size();
    assert_eq!(plan.demand.bytes(), expected);
    assert_eq!(plan.demand.allocations(), 7);
}

#[test]
fn exhausted_generation_refuses_before_admission_or_successor_allocation() {
    let mut provider = Prepaid(Some(ScalarPolicy));
    let mut source = unsafe {
        SuperBlock::<usize, usize, Prepaid<ScalarPolicy>>::new_with_funding(&mut provider)
    };
    source.txid = (TXID_MASK >> TXID_SHF) - 1;
    let result = without_allocations(|| {
        plan_insert::<usize, usize, ScalarPolicy>(
            &source,
            &7,
            MapCell::<usize, usize, Prepaid<ScalarPolicy>>::writer_allocation_layouts(),
        )
    });
    assert!(matches!(result, Err(PlanningError::Overflow)));
}

struct UnknownPayload;
impl NodeFunding for UnknownPayload {
    type Charge = Untracked;
    fn take_node_charge(&mut self, _: Layout) -> Untracked {
        Untracked
    }
}
impl NodeCloning<Box<usize>, Box<usize>> for UnknownPayload {
    fn clone_key(&mut self, _: &Box<usize>) -> Box<usize> {
        panic!("unplanned key copy")
    }
    fn clone_value(&mut self, _: &Box<usize>) -> Box<usize> {
        panic!("unplanned value copy")
    }
}
impl ClonePlanning<Box<usize>, Box<usize>> for UnknownPayload {
    fn plan_key(_: &Box<usize>, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Err(PlanningError::UnsupportedPayload)
    }
    fn plan_value(_: &Box<usize>, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Err(PlanningError::UnsupportedPayload)
    }
}

#[test]
fn unsupported_payload_returns_original_owners_without_calling_admission() {
    let map =
        BptreeMap::<Box<usize>, Box<usize>, Prepaid<UnknownPayload>>::try_new_with_node_custody(
            |_| Ok::<_, ()>(UnknownPayload),
        )
        .unwrap();
    let key = Box::new(7);
    let value = Box::new(21);
    let pointers = (&*key as *const usize, &*value as *const usize);
    let result = without_allocations(|| {
        map.try_insert_admitted(key, value, |_| -> Result<UnknownPayload, ()> {
            panic!("planning refusal must precede admission")
        })
    });
    let Err(((key, value), InsertAdmissionError::Planning(PlanningError::UnsupportedPayload))) =
        result
    else {
        panic!("original unsupported input must be returned")
    };
    assert_eq!((&*key as *const usize, &*value as *const usize), pointers);
    assert!(map.read().is_empty());
    assert!(!map.is_poisoned());
}

#[test]
fn initial_node_admission_refusal_constructs_no_root_or_reader() {
    let result = without_allocations(|| {
        BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|demand| {
            assert!(demand.bytes() > 0);
            assert_eq!(demand.allocations(), 3);
            Err::<ScalarPolicy, _>(17)
        })
    });
    assert!(matches!(result, Err(17)));
}

#[test]
fn retained_edits_keep_the_original_cursor_and_refused_tracking_then_publish_once() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let (mut owner, _) = map
        .try_insert_admitted(0, 0, |_| Ok::<_, ()>(ScalarPolicy))
        .unwrap_or_else(|_| panic!("first edit"));
    let cursor = owner.inner.as_ref() as *const _;
    for key in 1..128 {
        let original_tracking = owner.inner.as_ref().admitted_tracking();
        let ((refused, (same_key, same_value)), error) = without_allocations(|| {
            map.try_insert_owned_admitted(owner, key, key * 3, |_| Err::<ScalarPolicy, _>(19))
                .err()
                .expect("refused edit")
        });
        assert!(matches!(error, InsertAdmissionError::Refused(19)));
        assert_eq!(refused.inner.as_ref() as *const _, cursor);
        assert_eq!(
            refused.inner.as_ref().admitted_tracking(),
            original_tracking
        );
        assert_eq!((same_key, same_value), (key, key * 3));
        let (next, previous) = map
            .try_insert_owned_admitted(refused, same_key, same_value, |_| Ok::<_, ()>(ScalarPolicy))
            .unwrap_or_else(|_| panic!("retained edit"));
        assert!(previous.is_none());
        assert_eq!(next.inner.as_ref() as *const _, cursor);
        assert!(next.inner.as_ref().verify());
        assert!(map.read().is_empty());
        owner = next;
    }
    assert_eq!(owner.to_snapshot().len(), 128);
    for key in 0..128 {
        assert_eq!(owner.get(&key), Some(&(key * 3)));
    }
    without_allocations(|| {
        map.try_write_owned(owner)
            .unwrap_or_else(|_| panic!("original writer"))
            .commit();
    });
    assert_eq!(map.read().len(), 128);
}

#[test]
fn tracking_growth_checks_overflow_before_changing_demand_or_allocating() {
    let mut demand = AllocationDemand::new();
    assert!(
        without_allocations(|| plan_tracking_growth::<usize, usize, ScalarPolicy>(
            1,
            4,
            3,
            &mut demand
        ))
        .unwrap()
        .is_none()
    );
    assert_eq!(demand, AllocationDemand::new());
    let growth = without_allocations(|| {
        plan_tracking_growth::<usize, usize, ScalarPolicy>(2, 4, 3, &mut demand)
    })
    .unwrap()
    .unwrap();
    assert_eq!(growth.capacity, 8);
    assert_eq!(
        growth.layout,
        Layout::array::<*mut Node<usize, usize>>(8).unwrap()
    );
    assert_eq!(demand.bytes(), growth.layout.size());
    let original = demand;
    assert!(matches!(
        without_allocations(|| plan_tracking_growth::<usize, usize, ScalarPolicy>(
            usize::MAX,
            usize::MAX,
            1,
            &mut demand
        )),
        Err(PlanningError::Overflow)
    ));
    assert_eq!(demand, original);
    assert!(matches!(
        without_allocations(|| plan_tracking_growth::<usize, usize, ScalarPolicy>(
            0,
            0,
            usize::MAX,
            &mut demand
        )),
        Err(PlanningError::Overflow)
    ));
    assert_eq!(demand, original);
}

mod writer_start {
    //! No-edit writer preflight, original tree custody and provider cleanup refusal.

    use super::super::*;
    use crate::internals::bptree::node::allocation_tests::without_allocations;
    use crate::internals::bptree::node::{TXID_MASK, TXID_SHF};
    use std::cell::Cell;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    thread_local! {
        static PLANS: Cell<usize> = const { Cell::new(0) };
        static CLONES: Cell<usize> = const { Cell::new(0) };
        static DROPS: Cell<usize> = const { Cell::new(0) };
        static CAPTURE_LAYOUTS: Cell<bool> = const { Cell::new(false) };
        static LAYOUTS: Cell<[Option<Layout>; 4]> = const { Cell::new([None; 4]) };
    }

    struct Policy {
        panic_drop: bool,
    }
    impl NodeFunding for Policy {
        type Charge = Untracked;
        fn take_node_charge(&mut self, layout: Layout) -> Untracked {
            if CAPTURE_LAYOUTS.with(Cell::get) {
                LAYOUTS.with(|record| {
                    let mut layouts = record.get();
                    let slot = layouts.iter_mut().find(|entry| entry.is_none()).unwrap();
                    *slot = Some(layout);
                    record.set(layouts);
                });
            }
            Untracked
        }
    }
    impl NodeCloning<usize, usize> for Policy {
        fn clone_key(&mut self, key: &usize) -> usize {
            CLONES.with(|count| count.set(count.get() + 1));
            *key
        }
        fn clone_value(&mut self, value: &usize) -> usize {
            CLONES.with(|count| count.set(count.get() + 1));
            *value
        }
    }
    impl ClonePlanning<usize, usize> for Policy {
        fn plan_key(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
            PLANS.with(|count| count.set(count.get() + 1));
            Ok(())
        }
        fn plan_value(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
            PLANS.with(|count| count.set(count.get() + 1));
            Ok(())
        }
    }
    impl Drop for Policy {
        fn drop(&mut self) {
            DROPS.with(|count| count.set(count.get() + 1));
            assert!(!self.panic_drop, "unused provider cleanup failed");
        }
    }

    type Map = BptreeMap<usize, usize, Prepaid<Policy>>;

    fn policy() -> Policy {
        Policy { panic_drop: false }
    }

    fn populated(length: usize) -> Map {
        let map = Map::try_new_with_node_custody(|_| Ok::<_, ()>(policy())).unwrap();
        for key in 0..length {
            let (owner, _) = map
                .try_insert_admitted(key, key * 3, |_| Ok::<_, ()>(policy()))
                .unwrap_or_else(|_| panic!("fixture insertion"));
            map.try_write_owned(owner)
                .unwrap_or_else(|_| panic!("original fixture writer"))
                .commit();
        }
        map
    }

    #[test]
    fn start_plan_is_only_two_shells_and_empty_tracking_with_checked_generation() {
        let mut funding = Prepaid(Some(policy()));
        let mut source =
            unsafe { SuperBlock::<usize, usize, Prepaid<Policy>>::new_with_funding(&mut funding) };
        let shells = MapCell::<usize, usize, Prepaid<Policy>>::writer_allocation_layouts();
        let plan = without_allocations(|| {
            plan_writer_start::<usize, usize, Policy>(&source, shells).unwrap()
        });
        assert_eq!(
            plan.demand.bytes(),
            shells.cursor.size() + shells.reader.size()
        );
        assert_eq!(plan.demand.allocations(), 2);
        assert_eq!(
            plan.tracking_layout,
            Layout::array::<*mut Node<usize, usize>>(0).unwrap()
        );
        assert_eq!(checked_next_generation(source.txid), Some(source.txid + 1));
        for txid in [(TXID_MASK >> TXID_SHF) - 1, u64::MAX] {
            source.txid = txid;
            assert!(matches!(
                without_allocations(|| plan_writer_start::<usize, usize, Policy>(&source, shells)),
                Err(PlanningError::Overflow)
            ));
            assert_eq!(source.txid, txid);
        }
    }

    #[test]
    fn empty_and_populated_starts_keep_exact_tree_and_zero_buffers_without_payload_work() {
        for length in [0, 32] {
            let map = populated(length);
            let old = map.read();
            let root = old.inner.as_ref().get_root();
            let txid = old.get_txid();
            PLANS.with(|count| count.set(0));
            CLONES.with(|count| count.set(0));
            DROPS.with(|count| count.set(0));
            LAYOUTS.with(|layouts| layouts.set([None; 4]));
            CAPTURE_LAYOUTS.with(|capture| capture.set(true));
            let writer = map.try_write_admitted(|_| Ok::<_, ()>(policy())).unwrap();
            CAPTURE_LAYOUTS.with(|capture| capture.set(false));
            let shells = MapCell::<usize, usize, Prepaid<Policy>>::writer_allocation_layouts();
            let zero = Layout::array::<*mut Node<usize, usize>>(0).unwrap();
            assert_eq!(
                LAYOUTS.with(Cell::get),
                [
                    Some(zero),
                    Some(zero),
                    Some(shells.cursor),
                    Some(shells.reader)
                ]
            );
            assert_eq!(PLANS.with(Cell::get), 0);
            assert_eq!(CLONES.with(Cell::get), 0);
            assert_eq!(DROPS.with(Cell::get), 1);
            assert_eq!(writer.inner.as_ref().get_root(), root);
            assert_eq!(writer.inner.as_ref().get_txid(), txid + 1);
            assert_eq!(writer.inner.as_ref().admitted_tracking(), [(0, 0); 2]);
            assert_eq!(writer.len(), length);
            let cursor = writer.inner.as_ref() as *const _;
            let owner = without_allocations(|| writer.detach());
            assert_eq!(owner.inner.as_ref() as *const _, cursor);
            let writer = without_allocations(|| {
                map.try_write_owned(owner)
                    .unwrap_or_else(|_| panic!("same original cursor"))
            });
            assert_eq!(writer.inner.as_ref() as *const _, cursor);
            without_allocations(|| drop(writer));
            assert_eq!(map.read().inner.as_ref().get_root(), root);
            assert_eq!(map.read().get_txid(), txid);
            assert_eq!(map.read().len(), length);
            assert!(!map.is_poisoned());
        }
    }

    #[test]
    fn start_busy_and_admission_refusal_allocate_nothing_and_preserve_published_state() {
        let map = populated(1);
        let writer = map.try_write_admitted(|_| Ok::<_, ()>(policy())).unwrap();
        let busy =
            without_allocations(|| map.try_write_admitted::<()>(|_| panic!("busy admission")));
        assert!(matches!(busy, Err(InsertAdmissionError::Busy)));
        drop(writer);
        let refused = without_allocations(|| {
            map.try_write_admitted(|demand| {
                assert_eq!(demand.allocations(), 2);
                Err::<Policy, _>(19)
            })
        });
        assert!(matches!(refused, Err(InsertAdmissionError::Refused(19))));
        assert_eq!(map.read().get(&0), Some(&0));
        assert!(!map.is_poisoned());
        drop(map.try_write_admitted(|_| Ok::<_, ()>(policy())).unwrap());
    }

    #[test]
    fn exhausted_start_refuses_before_callback_or_allocation() {
        let mut funding = Prepaid(Some(policy()));
        let mut source =
            unsafe { SuperBlock::<usize, usize, Prepaid<Policy>>::new_with_funding(&mut funding) };
        source.txid = (TXID_MASK >> TXID_SHF) - 1;
        let root = source.root;
        let map = Map {
            inner: LinCowCell::new_charged(
                source,
                InitialCharges {
                    root: Untracked,
                    reader: Untracked,
                },
            ),
        };
        let result = without_allocations(|| {
            map.try_write_admitted::<()>(|_| panic!("generation refusal must precede admission"))
        });
        assert!(matches!(
            result,
            Err(InsertAdmissionError::Planning(PlanningError::Overflow))
        ));
        assert_eq!(map.read().inner.as_ref().get_root(), root);
        assert!(map.read().is_empty());
        assert!(!map.is_poisoned());
    }

    #[test]
    fn provider_drop_panic_poisoned_start_never_publishes_or_returns_a_writer() {
        let map = populated(1);
        let root = map.read().inner.as_ref().get_root();
        let txid = map.read().get_txid();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _writer = map.try_write_admitted(|_| Ok::<_, ()>(Policy { panic_drop: true }));
            }))
            .is_err()
        );
        assert!(map.is_poisoned());
        assert_eq!(map.read().inner.as_ref().get_root(), root);
        assert_eq!(map.read().get_txid(), txid);
        assert_eq!(map.read().get(&0), Some(&0));
        let refused =
            without_allocations(|| map.try_write_admitted::<()>(|_| panic!("poisoned admission")));
        assert!(matches!(refused, Err(InsertAdmissionError::Poisoned)));
    }

    #[test]
    fn started_original_writer_admits_later_growth_and_checkpoint_abort_without_new_credit() {
        let map = populated(1);
        let mut writer = map.try_write_admitted(|_| Ok::<_, ()>(policy())).unwrap();
        let cursor = writer.inner.as_ref() as *const _;
        let root = writer.inner.as_ref().get_root();
        let txid = writer.inner.as_ref().get_txid();
        {
            let mut checkpoint = without_allocations(|| writer.checkpoint().unwrap());
            checkpoint
                .try_insert_admitted(1, 3, |_| Ok::<_, ()>(policy()))
                .unwrap();
            assert_eq!(checkpoint.get(&1), Some(&3));
            without_allocations(|| drop(checkpoint));
        }
        assert_eq!(writer.inner.as_ref() as *const _, cursor);
        assert_eq!(writer.inner.as_ref().get_root(), root);
        assert_eq!(writer.inner.as_ref().get_txid(), txid);
        assert_eq!(writer.inner.as_ref().admitted_tracking(), [(0, 0); 2]);
        assert_eq!(writer.get(&1), None);
        writer
            .try_insert_admitted(2, 6, |_| Ok::<_, ()>(policy()))
            .unwrap();
        assert_eq!(writer.inner.as_ref() as *const _, cursor);
        assert_eq!(map.read().get(&2), None);
        without_allocations(|| writer.commit());
        assert_eq!(map.read().get(&2), Some(&6));
    }
}

#[test]
fn prepared_writer_and_checkpoint_planning_refuse_without_copying_original_inputs() {
    let map =
        BptreeMap::<Box<usize>, Box<usize>, Prepaid<UnknownPayload>>::try_new_with_node_custody(
            |_| Ok::<_, ()>(UnknownPayload),
        )
        .unwrap();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(UnknownPayload))
        .unwrap();
    let cursor = writer.inner.as_ref() as *const _;
    let key = Box::new(7);
    let value = Box::new(21);
    let pointers = (&*key as *const usize, &*value as *const usize);
    let ((key, value), error) = without_allocations(|| {
        writer
            .prepare_insert_admitted(key, value)
            .err()
            .expect("unknown demand")
    });
    assert_eq!(error, PlanningError::UnsupportedPayload);
    assert_eq!((&*key as *const usize, &*value as *const usize), pointers);
    assert_eq!(writer.inner.as_ref() as *const _, cursor);
    assert!(writer.is_empty());
    let mut checkpoint = writer.checkpoint().unwrap();
    let ((key, value), error) = without_allocations(|| {
        checkpoint
            .prepare_insert_admitted(key, value)
            .err()
            .expect("same unknown demand")
    });
    assert_eq!(error, PlanningError::UnsupportedPayload);
    assert_eq!((&*key as *const usize, &*value as *const usize), pointers);
    assert!(checkpoint.is_empty());
    without_allocations(|| checkpoint.apply());
    assert_eq!(writer.inner.as_ref() as *const _, cursor);
    assert!(writer.is_empty());
    without_allocations(|| writer.commit());
    assert!(map.read().is_empty());
    assert!(!map.is_poisoned());
}

impl NodeCloning<usize, Option<usize>> for ScalarPolicy {
    fn clone_key(&mut self, key: &usize) -> usize {
        *key
    }
    fn clone_value(&mut self, value: &Option<usize>) -> Option<usize> {
        *value
    }
}
impl ClonePlanning<usize, Option<usize>> for ScalarPolicy {
    fn plan_key(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &Option<usize>, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}
impl NodeCloning<Box<usize>, Option<Box<usize>>> for UnknownPayload {
    fn clone_key(&mut self, _: &Box<usize>) -> Box<usize> {
        panic!("unplanned key copy")
    }
    fn clone_value(&mut self, _: &Option<Box<usize>>) -> Option<Box<usize>> {
        panic!("unplanned optional copy")
    }
}
impl ClonePlanning<Box<usize>, Option<Box<usize>>> for UnknownPayload {
    fn plan_key(_: &Box<usize>, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Err(PlanningError::UnsupportedPayload)
    }
    fn plan_value(_: &Option<Box<usize>>, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Err(PlanningError::UnsupportedPayload)
    }
}

#[test]
fn copied_key_planning_refusal_keeps_the_source_and_original_owned_value() {
    let map =
        BptreeMap::<Box<usize>, Box<usize>, Prepaid<UnknownPayload>>::try_new_with_node_custody(
            |_| Ok::<_, ()>(UnknownPayload),
        )
        .unwrap();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(UnknownPayload))
        .unwrap();
    let key = Box::new(7);
    let value = Box::new(21);
    let pointers = (&*key as *const usize, &*value as *const usize);
    let (value, error) = without_allocations(|| {
        writer
            .prepare_key_copy_insert_admitted(&key, value)
            .err()
            .expect("unknown demand")
    });
    assert_eq!(error, PlanningError::UnsupportedPayload);
    assert_eq!((&*key as *const usize, &*value as *const usize), pointers);
    let mut checkpoint = writer.checkpoint().unwrap();
    let (value, error) = without_allocations(|| {
        checkpoint
            .prepare_key_copy_insert_admitted(&key, value)
            .err()
            .expect("unknown demand")
    });
    assert_eq!(error, PlanningError::UnsupportedPayload);
    assert_eq!((&*key as *const usize, &*value as *const usize), pointers);
    without_allocations(|| checkpoint.apply());
    assert!(writer.is_empty());
    without_allocations(|| writer.commit());
    assert!(!map.is_poisoned());
}

#[test]
fn optional_copy_planning_refusal_does_not_clone_sources_or_edit_either_guard() {
    let map = BptreeMap::<Box<usize>, Option<Box<usize>>, Prepaid<UnknownPayload>>::try_new_with_node_custody(|_| Ok::<_, ()>(UnknownPayload)).unwrap();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(UnknownPayload))
        .unwrap();
    let key = Box::new(7);
    let value = Box::new(21);
    for source in [None, Some(&value)] {
        let error = without_allocations(|| {
            writer
                .prepare_optional_copy_insert_admitted(&key, source)
                .err()
                .expect("unknown demand")
        });
        assert_eq!(error, PlanningError::UnsupportedPayload);
        let mut checkpoint = writer.checkpoint().unwrap();
        let error = without_allocations(|| {
            checkpoint
                .prepare_optional_copy_insert_admitted(&key, source)
                .err()
                .expect("unknown demand")
        });
        assert_eq!(error, PlanningError::UnsupportedPayload);
        without_allocations(|| checkpoint.apply());
    }
    assert_eq!((*key, *value), (7, 21));
    assert!(writer.is_empty());
    without_allocations(|| writer.commit());
    assert!(!map.is_poisoned());
}

#[test]
fn optional_none_is_an_existing_preimage_and_checkpoint_abort_restores_it() {
    let map =
        BptreeMap::<usize, Option<usize>, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    let key = 7;
    let value = 21;
    let prepared = without_allocations(|| {
        writer
            .prepare_optional_copy_insert_admitted(&key, None)
            .unwrap()
    });
    assert!(prepared.demand().bytes() > 0);
    assert_eq!(prepared.execute(ScalarPolicy), None);
    assert_eq!(writer.get(&key), Some(&None));
    {
        let mut child = writer.checkpoint().unwrap();
        let prepared = without_allocations(|| {
            child
                .prepare_optional_copy_insert_admitted(&key, Some(&value))
                .unwrap()
        });
        assert_eq!(prepared.execute(ScalarPolicy), Some(None));
        assert_eq!(child.get(&key), Some(&Some(value)));
        assert_eq!(child.get_before(&key), Some(&None));
        without_allocations(|| drop(child));
    }
    assert_eq!(writer.get(&key), Some(&None));
    without_allocations(|| writer.commit());
    assert_eq!(map.read().get(&key), Some(&None));
}

#[test]
fn original_preparation_retains_key_and_preimage_through_dependent_copy_then_cancel() {
    let source =
        BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap();
    let mut original = source
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    original
        .try_insert_admitted(7, 21, |_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    let target =
        BptreeMap::<usize, Option<usize>, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap();
    let mut undo = target
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    let current = without_allocations(|| {
        original
            .prepare_insert_admitted(7, 99)
            .unwrap_or_else(|_| panic!("scalar plan"))
    });
    assert_eq!(current.input_key(), &7);
    assert_eq!(current.previous_value(), Some(&21));
    let copied = without_allocations(|| {
        undo.prepare_optional_copy_insert_admitted(current.input_key(), current.previous_value())
            .unwrap()
    });
    assert_eq!(copied.execute(ScalarPolicy), None);
    assert_eq!(without_allocations(|| current.into_input()), (7, 99));
    assert_eq!(original.get(&7), Some(&21));
    assert_eq!(undo.get(&7), Some(&Some(21)));
    let missing = without_allocations(|| {
        original
            .prepare_insert_admitted(8, 88)
            .unwrap_or_else(|_| panic!("scalar plan"))
    });
    assert_eq!(missing.previous_value(), None);
    assert_eq!(without_allocations(|| missing.into_input()), (8, 88));
}

#[test]
fn key_copy_cancel_returns_original_value_and_checkpoint_copy_uses_same_cursor() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    let key = 7;
    let prepared = without_allocations(|| {
        writer
            .prepare_key_copy_insert_admitted(&key, 21)
            .unwrap_or_else(|_| panic!("scalar plan"))
    });
    assert!(prepared.demand().bytes() > 0);
    assert_eq!(without_allocations(|| prepared.into_value()), 21);
    assert!(writer.is_empty());
    let mut child = writer.checkpoint().unwrap();
    let prepared = without_allocations(|| {
        child
            .prepare_key_copy_insert_admitted(&key, 21)
            .unwrap_or_else(|_| panic!("scalar plan"))
    });
    assert_eq!(prepared.execute(ScalarPolicy), None);
    assert_eq!(child.get(&key), Some(&21));
    without_allocations(|| child.apply());
    assert_eq!(writer.get(&key), Some(&21));
    without_allocations(|| writer.commit());
    assert_eq!(map.read().get(&key), Some(&21));
}

// A shared mutable backing can change demand while only immutably borrowed.
// This policy deliberately refuses a copy rather than claiming a stale bound.
type SharedBytes = std::sync::Arc<std::sync::Mutex<Vec<u8>>>;
struct UnstableCopy;
impl NodeFunding for UnstableCopy {
    type Charge = Untracked;
    fn take_node_charge(&mut self, _: Layout) -> Untracked {
        Untracked
    }
}
impl NodeCloning<usize, SharedBytes> for UnstableCopy {
    fn clone_key(&mut self, key: &usize) -> usize {
        *key
    }
    fn clone_value(&mut self, _: &SharedBytes) -> SharedBytes {
        panic!("unstable copy admitted")
    }
}
impl NodeCloning<usize, Option<SharedBytes>> for UnstableCopy {
    fn clone_key(&mut self, key: &usize) -> usize {
        *key
    }
    fn clone_value(&mut self, _: &Option<SharedBytes>) -> Option<SharedBytes> {
        panic!("unstable optional copy admitted")
    }
}
impl ClonePlanning<usize, SharedBytes> for UnstableCopy {
    fn plan_key(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &SharedBytes, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Err(PlanningError::UnsupportedPayload)
    }
}
impl ClonePlanning<usize, Option<SharedBytes>> for UnstableCopy {
    fn plan_key(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(
        value: &Option<SharedBytes>,
        _: &mut AllocationDemand,
    ) -> Result<(), PlanningError> {
        if value.is_some() {
            Err(PlanningError::UnsupportedPayload)
        } else {
            Ok(())
        }
    }
}
#[test]
fn incoming_shared_mutable_preimage_refuses_when_its_borrow_cannot_freeze_copy_demand() {
    let map =
        BptreeMap::<usize, Option<SharedBytes>, Prepaid<UnstableCopy>>::try_new_with_node_custody(
            |_| Ok::<_, ()>(UnstableCopy),
        )
        .unwrap();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(UnstableCopy))
        .unwrap();
    let source = std::sync::Arc::new(std::sync::Mutex::new(vec![1]));
    for length in [1, 1024] {
        source.lock().unwrap().resize(length, 2);
        let error = without_allocations(|| {
            writer
                .prepare_optional_copy_insert_admitted(&7, Some(&source))
                .err()
                .expect("unstable incoming value must refuse")
        });
        assert_eq!(error, PlanningError::UnsupportedPayload);
        assert_eq!(source.lock().unwrap().len(), length);
        assert_eq!(std::sync::Arc::strong_count(&source), 1);
        assert!(writer.is_empty());
    }
    let absent = without_allocations(|| {
        writer
            .prepare_optional_copy_insert_admitted(&7, None)
            .unwrap()
    });
    assert!(absent.execute(UnstableCopy).is_none());
    assert!(matches!(writer.get(&7), Some(None)));
    without_allocations(|| writer.commit());
    assert!(!map.is_poisoned());
}
