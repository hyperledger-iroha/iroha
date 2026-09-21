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
fn acquired_admission_refusal_retains_actual_writer_and_deferred_release() {
    use crate::release::ReleaseNotification;
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Waker},
    };
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let source = ReleaseNotification::default();
    let mut wait = source.observe().wait_for_release();
    let acquired = without_allocations(|| map.try_acquire_writer().unwrap());
    let (acquired, (input, error)) = without_allocations(|| {
        source
            .poisoning_guard(acquired)
            .try_map_preserving_release(|acquired| {
                acquired
                    .try_insert_admitted_with_footprint(7, 21, |existing, additional| {
                        assert!(existing.bytes() > 0 && additional.bytes() > 0);
                        Err::<ScalarPolicy, _>(17)
                    })
                    .map_err(|(acquired, input, error)| (acquired, (input, error)))
            })
            .err()
            .expect("original admission refusal")
    });
    assert_eq!(input, (7, 21));
    assert!(matches!(error, MapAdmissionError::Refused(17)));
    assert!(map.try_acquire_writer().is_none());
    assert!(Pin::new(&mut wait)
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_pending());
    let ((), release) = without_allocations(|| acquired.release_deferred(drop));
    assert!(map.try_acquire_writer().is_some());
    assert!(Pin::new(&mut wait)
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_pending());
    drop(release);
    assert!(Pin::new(&mut wait)
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_ready());
    assert!(map.read().is_empty());
}

#[test]
fn acquired_admission_busy_poison_and_unwind_preserve_real_custody() {
    use crate::release::ReleaseNotification;
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Waker},
    };
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let source = ReleaseNotification::default();
    let observation = source.observe();
    let mut wait = observation.clone().wait_for_release();
    let acquired = source.poisoning_guard(map.try_acquire_writer().unwrap());
    assert!(without_allocations(|| map.try_acquire_writer()).is_none());
    assert!(Pin::new(&mut wait)
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_pending());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = acquired.try_map_preserving_release(|acquired| {
            acquired
                .try_insert_admitted_with_footprint(7, 21, |_, _| -> Result<ScalarPolicy, ()> {
                    panic!("admission callback failed while holding actual writer");
                })
                .map_err(|(acquired, input, error)| (acquired, (input, error)))
        });
    }))
    .is_err());
    assert!(map.is_poisoned() && observation.is_poisoned());
    assert!(Pin::new(&mut wait)
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_ready());
    let acquired = without_allocations(|| map.try_acquire_writer().unwrap());
    let (acquired, input, error) = without_allocations(|| {
        acquired
            .try_insert_admitted_with_footprint(7, 21, |_, _| -> Result<ScalarPolicy, ()> {
                panic!("poison precedes admission");
            })
            .err()
            .expect("retain the actual poisoned guard")
    });
    assert_eq!(input, (7, 21));
    assert!(matches!(error, MapAdmissionError::Poisoned));
    assert!(map.try_acquire_writer().is_none());
    drop(acquired);
    assert!(map.try_acquire_writer().is_some());
}

#[test]
fn acquired_admission_success_and_planning_refusal_preserve_original_input() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let acquired = map.try_acquire_writer().unwrap();
    let (writer, previous) = acquired
        .try_insert_admitted_with_footprint(7, 21, |_, _| Ok::<_, ()>(ScalarPolicy))
        .unwrap_or_else(|_| panic!("complete original admission"));
    assert_eq!(previous, None);
    assert!(map.try_acquire_writer().is_none());
    assert!(map.read().is_empty(), "successor remains private");
    let pointer = writer.get(&7).map(std::ptr::from_ref);
    let owned = writer.detach();
    assert_eq!(owned.get(&7).map(std::ptr::from_ref), pointer);
    map.try_write_owned(owned)
        .unwrap_or_else(|_| panic!("same predecessor"))
        .commit();
    assert_eq!(map.read().get(&7), Some(&21));

    let map =
        BptreeMap::<Box<usize>, Box<usize>, Prepaid<UnknownPayload>>::try_new_with_node_custody(
            |_| Ok::<_, ()>(UnknownPayload),
        )
        .unwrap();
    let key = Box::new(7);
    let value = Box::new(21);
    let pointers = (std::ptr::from_ref(&*key), std::ptr::from_ref(&*value));
    let acquired = map.try_acquire_writer().unwrap();
    let (acquired, (key, value), error) = without_allocations(|| {
        acquired
            .insert_with_source(
                key,
                value,
                |_, _| -> Result<UnknownPayload, MapAdmissionError<()>> {
                    panic!("unsupported payload planning must precede admission");
                },
            )
            .err()
            .expect("original planning refusal")
    });
    assert!(matches!(
        error,
        MapAdmissionError::Planning(PlanningError::UnsupportedPayload)
    ));
    assert_eq!(
        (std::ptr::from_ref(&*key), std::ptr::from_ref(&*value)),
        pointers
    );
    assert!(map.try_acquire_writer().is_none());
    drop(acquired);
    assert!(map.try_acquire_writer().is_some());
    assert!(map.read().is_empty());
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
    let Err(((key, value), MapAdmissionError::Planning(PlanningError::UnsupportedPayload))) =
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
        assert!(matches!(error, MapAdmissionError::Refused(19)));
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

#[test]
fn held_writer_demand_is_allocation_free_and_matches_admission_before_any_growth() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let (owner, _) = map
        .try_insert_admitted(0, 0, |_| Ok::<_, ()>(ScalarPolicy))
        .unwrap_or_else(|_| panic!("first edit"));
    let mut writer = map
        .try_write_owned(owner)
        .unwrap_or_else(|_| panic!("original writer"));
    let original_cursor = writer.inner.as_ref() as *const _;
    let original_root = writer.inner.as_ref().get_root();
    let original_tracking = writer.inner.as_ref().admitted_tracking();
    assert_eq!(original_tracking, [(1, 3), (1, 1)]);
    let demand = without_allocations(|| writer.insertion_demand(&1).unwrap());
    let expected = 2 * Layout::new::<CachePadded<Leaf<usize, usize>>>().size()
        + Layout::new::<CachePadded<Branch<usize, usize>>>().size()
        + Layout::array::<*mut Node<usize, usize>>(6).unwrap().size()
        + Layout::array::<*mut Node<usize, usize>>(2).unwrap().size();
    assert_eq!(demand.bytes(), expected);
    assert_eq!(demand.allocations(), 5);
    without_allocations(|| {
        for _ in 0..3 {
            assert_eq!(writer.insertion_demand(&1), Ok(demand));
        }
        let refused = writer.try_insert_admitted(1, 3, |actual| {
            assert_eq!(actual, demand);
            Err::<ScalarPolicy, _>(23)
        });
        assert!(matches!(
            refused,
            Err(((1, 3), MapAdmissionError::Refused(23)))
        ));
        assert_eq!(writer.inner.as_ref() as *const _, original_cursor);
        assert_eq!(writer.inner.as_ref().get_root(), original_root);
        assert_eq!(writer.inner.as_ref().admitted_tracking(), original_tracking);
        assert_eq!(writer.len(), 1);
    });
    writer
        .try_insert_admitted(1, 3, |actual| {
            assert_eq!(actual, demand);
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap_or_else(|_| panic!("retry original edit"));
    assert_eq!(writer.inner.as_ref() as *const _, original_cursor);
    assert_eq!(writer.get(&1), Some(&3));
    assert!(map.read().is_empty());
}

#[test]
fn nested_checkpoint_demand_tracks_retirement_growth_and_exact_abort_restoration() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let (owner, _) = map
        .try_insert_admitted(0, 0, |_| Ok::<_, ()>(ScalarPolicy))
        .unwrap_or_else(|_| panic!("first edit"));
    let mut writer = map
        .try_write_owned(owner)
        .unwrap_or_else(|_| panic!("original writer"));
    let before = without_allocations(|| writer.insertion_demand(&1).unwrap());
    let original_root = writer.inner.as_ref().get_root();
    let original_tracking = writer.inner.as_ref().admitted_tracking();
    let mut outer = without_allocations(|| writer.checkpoint().unwrap());
    assert_eq!(
        without_allocations(|| outer.insertion_demand(&1)),
        Ok(before)
    );
    outer
        .try_insert_admitted(1, 3, |actual| {
            assert_eq!(actual, before);
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap_or_else(|_| panic!("outer edit"));
    assert_eq!(outer.inner.as_ref().admitted_tracking(), [(2, 6), (2, 2)]);
    let mut child = without_allocations(|| outer.checkpoint().unwrap());
    let child_root = child.inner.as_ref().get_root();
    let expected = 2 * Layout::new::<CachePadded<Leaf<usize, usize>>>().size()
        + Layout::new::<CachePadded<Branch<usize, usize>>>().size()
        + Layout::array::<*mut Node<usize, usize>>(4).unwrap().size();
    let demand = without_allocations(|| child.insertion_demand(&2).unwrap());
    assert_eq!(demand.bytes(), expected);
    assert_eq!(demand.allocations(), 4);
    without_allocations(|| {
        let refused = child.try_insert_admitted(2, 6, |actual| {
            assert_eq!(actual, demand);
            Err::<ScalarPolicy, _>(29)
        });
        assert!(matches!(
            refused,
            Err(((2, 6), MapAdmissionError::Refused(29)))
        ));
        assert_eq!(child.inner.as_ref().get_root(), child_root);
        assert_eq!(child.inner.as_ref().admitted_tracking(), [(2, 6), (2, 2)]);
        assert_eq!(child.get_before(&1), Some(&3));
    });
    child
        .try_insert_admitted(2, 6, |actual| {
            assert_eq!(actual, demand);
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap_or_else(|_| panic!("nested edit"));
    child.apply();
    without_allocations(|| drop(outer));
    assert_eq!(writer.inner.as_ref().get_root(), original_root);
    assert_eq!(writer.inner.as_ref().admitted_tracking(), original_tracking);
    assert_eq!(
        without_allocations(|| writer.insertion_demand(&1)),
        Ok(before)
    );
    assert_eq!(writer.len(), 1);
}

#[test]
fn stale_observed_demand_never_skips_replanning_after_a_private_edit() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let (owner, _) = map
        .try_insert_admitted(0, 0, |_| Ok::<_, ()>(ScalarPolicy))
        .unwrap_or_else(|_| panic!("first edit"));
    let mut writer = map
        .try_write_owned(owner)
        .unwrap_or_else(|_| panic!("original writer"));
    let stale = without_allocations(|| writer.insertion_demand(&2).unwrap());
    writer
        .try_insert_admitted(1, 3, |_| Ok::<_, ()>(ScalarPolicy))
        .unwrap_or_else(|_| panic!("intervening edit"));
    let fresh = without_allocations(|| writer.insertion_demand(&2).unwrap());
    assert_eq!(stale.allocations(), fresh.allocations() + 2);
    assert_eq!(
        stale.bytes() - fresh.bytes(),
        Layout::array::<*mut Node<usize, usize>>(8).unwrap().size()
    );
    let refused = without_allocations(|| {
        writer.try_insert_admitted(2, 6, |actual| {
            assert_eq!(actual, fresh);
            assert_ne!(actual, stale);
            Err::<ScalarPolicy, _>(31)
        })
    });
    assert!(matches!(
        refused,
        Err(((2, 6), MapAdmissionError::Refused(31)))
    ));
    assert_eq!(writer.get(&1), Some(&3));
    assert_eq!(writer.get(&2), None);
    writer
        .try_insert_admitted(2, 6, |actual| {
            assert_eq!(actual, fresh);
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap_or_else(|_| panic!("fresh admission"));
    writer.commit();
    assert_eq!(map.read().len(), 3);
}

struct SizedPayloadPolicy;
impl NodeFunding for SizedPayloadPolicy {
    type Charge = Untracked;
    fn take_node_charge(&mut self, _: Layout) -> Untracked {
        Untracked
    }
}
impl NodeCloning<usize, Box<[u8]>> for SizedPayloadPolicy {
    fn clone_key(&mut self, key: &usize) -> usize {
        *key
    }
    fn clone_value(&mut self, value: &Box<[u8]>) -> Box<[u8]> {
        value.clone()
    }
}
impl ClonePlanning<usize, Box<[u8]>> for SizedPayloadPolicy {
    fn plan_key(key: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        if *key == usize::MAX {
            Err(PlanningError::UnsupportedPayload)
        } else {
            Ok(())
        }
    }
    fn plan_value(value: &Box<[u8]>, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        demand.add_layout(Layout::array::<u8>(value.len()).map_err(|_| PlanningError::Overflow)?)
    }
}

#[test]
fn held_demand_rechecks_actual_nested_layouts_and_returns_unsupported_original_input() {
    let map =
        BptreeMap::<usize, Box<[u8]>, Prepaid<SizedPayloadPolicy>>::try_new_with_node_custody(
            |_| Ok::<_, ()>(SizedPayloadPolicy),
        )
        .unwrap();
    let (owner, _) = map
        .try_insert_admitted(0, vec![1].into_boxed_slice(), |_| {
            Ok::<_, ()>(SizedPayloadPolicy)
        })
        .unwrap_or_else(|_| panic!("first edit"));
    let mut writer = map
        .try_write_owned(owner)
        .unwrap_or_else(|_| panic!("original writer"));
    let stale = without_allocations(|| writer.insertion_demand(&1).unwrap());
    writer
        .try_insert_admitted(0, vec![2; 129].into_boxed_slice(), |_| {
            Ok::<_, ()>(SizedPayloadPolicy)
        })
        .unwrap_or_else(|_| panic!("replace nested allocation"));
    let fresh = without_allocations(|| writer.insertion_demand(&1).unwrap());
    let expected = 2 * Layout::new::<CachePadded<Leaf<usize, Box<[u8]>>>>().size()
        + Layout::new::<CachePadded<Branch<usize, Box<[u8]>>>>().size()
        + 129;
    assert_eq!(fresh.bytes(), expected);
    assert_eq!(fresh.allocations(), 4);
    assert_ne!(fresh, stale);
    let current_value = writer.get(&0).unwrap().as_ptr();
    let current_tracking = writer.inner.as_ref().admitted_tracking();
    let input = vec![7; 17].into_boxed_slice();
    let original_input = input.as_ptr();
    let refused = without_allocations(|| {
        assert_eq!(
            writer.insertion_demand(&usize::MAX),
            Err(PlanningError::UnsupportedPayload)
        );
        writer.try_insert_admitted(usize::MAX, input, |_| -> Result<SizedPayloadPolicy, ()> {
            panic!("unsupported planning must precede admission")
        })
    });
    let Err(((key, input), MapAdmissionError::Planning(PlanningError::UnsupportedPayload))) =
        refused
    else {
        panic!("expected original input and planning refusal")
    };
    assert_eq!(key, usize::MAX);
    assert_eq!(input.as_ptr(), original_input);
    assert_eq!(writer.get(&0).unwrap().as_ptr(), current_value);
    assert_eq!(writer.inner.as_ref().admitted_tracking(), current_tracking);
    let mut checkpoint = writer.checkpoint().unwrap();
    assert_eq!(
        without_allocations(|| checkpoint.insertion_demand(&1)),
        Ok(fresh)
    );
    let refused = without_allocations(|| {
        checkpoint.try_insert_admitted(1, input, |actual| {
            assert_eq!(actual, fresh);
            Err::<SizedPayloadPolicy, _>(37)
        })
    });
    let Err(((1, input), MapAdmissionError::Refused(37))) = refused else {
        panic!("exact admitted refusal")
    };
    assert_eq!(input.as_ptr(), original_input);
    assert_eq!(checkpoint.get(&0).unwrap().as_ptr(), current_value);
}

#[test]
fn whole_operation_demand_sum_checks_both_counts_and_preserves_refused_total() {
    without_allocations(|| {
        let mut current = AllocationDemand::new();
        current.add_layout(Layout::new::<usize>()).unwrap();
        let mut undo = AllocationDemand::new();
        undo.add_layout(Layout::array::<usize>(3).unwrap()).unwrap();
        undo.add_layout(Layout::new::<()>()).unwrap();
        let before = current;
        current.add_demand(undo).unwrap();
        assert_eq!(current.bytes(), 4 * std::mem::size_of::<usize>());
        assert_eq!(current.allocations(), 2);
        assert_eq!(before.allocations(), 1);
        assert_eq!(undo.allocations(), 1);
        let mut oversized = AllocationDemand::new();
        let large = Layout::from_size_align(isize::MAX as usize, 1).unwrap();
        oversized.add_layout(large).unwrap();
        oversized.add_layout(large).unwrap();
        let original = current;
        assert_eq!(current.add_demand(oversized), Err(PlanningError::Overflow));
        assert_eq!(current, original);
        assert_eq!(oversized.bytes(), usize::MAX - 1);
        assert_eq!(oversized.allocations(), 2);
    });
}

#[test]
fn empty_writer_acquisition_funds_only_shells_and_retains_the_original_root() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let source = map.read();
    let root = source.inner.get_root();
    let shells = MapCell::<usize, usize, Prepaid<ScalarPolicy>>::writer_allocation_layouts();
    let expected = AllocationDemand {
        bytes: shells.cursor.size() + shells.reader.size(),
        allocations: 2,
    };
    assert!(matches!(
        without_allocations(|| map.try_write_admitted(|actual| {
            assert_eq!(actual, expected);
            Err::<ScalarPolicy, _>(41)
        })),
        Err(MapAdmissionError::Refused(41))
    ));
    let writer = map
        .try_write_admitted(|actual| {
            assert_eq!(actual, expected);
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap();
    assert_eq!(writer.inner.as_ref().get_root(), root);
    assert_eq!(writer.inner.as_ref().admitted_tracking(), [(0, 0); 2]);
    without_allocations(|| {
        assert!(matches!(
            map.try_write_admitted(|_| -> Result<ScalarPolicy, ()> { panic!("busy") }),
            Err(MapAdmissionError::Busy)
        ));
        assert!(matches!(
            map.try_clear_admitted(|_| -> Result<ScalarPolicy, ()> { panic!("busy") }),
            Err(MapAdmissionError::Busy)
        ));
        let retained = writer.detach();
        assert_eq!(retained.inner.as_ref().get_root(), root);
        map.try_write_owned(retained)
            .unwrap_or_else(|_| panic!("original owner"))
            .commit();
    });
    assert_eq!(map.read().inner.get_root(), root);
    assert!(source.is_empty());
}

#[test]
fn nested_writer_and_clear_callbacks_hold_both_original_locks_before_joint_refusal() {
    let current =
        BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap();
    let undo = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let mut undo_writer = undo
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    undo_writer
        .try_insert_admitted(3, 7, |_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    undo_writer.commit();
    let mut calls = 0;
    let refused = without_allocations(|| {
        current.try_write_admitted(|mut total| {
            let refusal = undo.try_clear_admitted(|clear| {
                calls += 1;
                total.add_demand(clear).unwrap();
                assert_eq!(total.allocations(), 7);
                assert!(matches!(
                    current.try_write_admitted(|_| -> Result<ScalarPolicy, ()> {
                        panic!("outer original lock must remain held")
                    }),
                    Err(MapAdmissionError::Busy)
                ));
                assert!(matches!(
                    undo.try_write_admitted(|_| -> Result<ScalarPolicy, ()> {
                        panic!("inner original lock must remain held")
                    }),
                    Err(MapAdmissionError::Busy)
                ));
                Err::<ScalarPolicy, _>(43)
            });
            assert!(matches!(refusal, Err(MapAdmissionError::Refused(43))));
            Err::<ScalarPolicy, _>(43)
        })
    });
    assert!(matches!(refused, Err(MapAdmissionError::Refused(43))));
    assert_eq!(calls, 1);
    assert!(current.read().is_empty());
    assert_eq!(undo.read().get(&3), Some(&7));
    let mut held_undo = None;
    let current_writer = current
        .try_write_admitted(|mut total| {
            held_undo = Some(
                undo.try_clear_admitted(|clear| {
                    total.add_demand(clear).unwrap();
                    calls += 1;
                    Ok::<_, ()>(ScalarPolicy)
                })
                .unwrap(),
            );
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap();
    assert_eq!(calls, 2);
    assert!(held_undo.as_ref().unwrap().is_empty());
    assert_eq!(undo.read().get(&3), Some(&7));
    without_allocations(|| {
        drop(current_writer);
        drop(held_undo);
    });
    assert_eq!(undo.read().get(&3), Some(&7));
}

struct CountedValue {
    value: Box<usize>,
    drops: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}
impl Clone for CountedValue {
    fn clone(&self) -> Self {
        panic!("clear must never clone a nested payload")
    }
}
impl Drop for CountedValue {
    fn drop(&mut self) {
        self.drops.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}
struct NoValueCopies;
impl NodeFunding for NoValueCopies {
    type Charge = Untracked;
    fn take_node_charge(&mut self, _: Layout) -> Untracked {
        Untracked
    }
}
impl NodeCloning<usize, CountedValue> for NoValueCopies {
    fn clone_key(&mut self, key: &usize) -> usize {
        *key
    }
    fn clone_value(&mut self, _: &CountedValue) -> CountedValue {
        panic!("unexpected payload copy")
    }
}
impl ClonePlanning<usize, CountedValue> for NoValueCopies {
    fn plan_key(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &CountedValue, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        demand.add_layout(Layout::new::<usize>())
    }
}

#[test]
fn clear_retires_the_entire_multilevel_tree_only_after_original_reader_release() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let drops = Arc::new(AtomicUsize::new(0));
    let map =
        BptreeMap::<usize, CountedValue, Prepaid<NoValueCopies>>::try_new_with_node_custody(|_| {
            Ok::<_, ()>(NoValueCopies)
        })
        .unwrap();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(NoValueCopies))
        .unwrap();
    for key in 0..128 {
        writer
            .try_insert_admitted(
                key,
                CountedValue {
                    value: Box::new(key * 3),
                    drops: drops.clone(),
                },
                |_| Ok::<_, ()>(NoValueCopies),
            )
            .unwrap_or_else(|_| panic!("same-generation insertion"));
    }
    writer.commit();
    let reader = map.read();
    let original = &*reader.get(&17).unwrap().value as *const usize;
    let cleared = map
        .try_clear_admitted(|demand| {
            assert!(demand.allocations() >= 5);
            Ok::<_, ()>(NoValueCopies)
        })
        .unwrap();
    assert!(cleared.is_empty());
    assert_eq!(reader.len(), 128);
    assert_eq!(&*reader.get(&17).unwrap().value as *const usize, original);
    without_allocations(|| cleared.commit());
    assert!(map.read().is_empty());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    without_allocations(|| drop(reader));
    assert_eq!(drops.load(Ordering::SeqCst), 128);
    without_allocations(|| drop(map));
    crate::internals::bptree::node::assert_released();
}

#[test]
fn checkpoint_clear_refusal_and_nested_apply_preserve_exact_outer_rollback() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    for key in 0..96 {
        writer
            .try_insert_admitted(key, key * 3, |_| Ok::<_, ()>(ScalarPolicy))
            .unwrap();
    }
    let original_root = writer.inner.as_ref().get_root();
    let original_tracking = writer.inner.as_ref().admitted_tracking();
    let pointer = writer.get(&17).unwrap() as *const usize;
    let demand = without_allocations(|| writer.clear_demand().unwrap());
    assert!(matches!(
        without_allocations(|| writer.try_clear_admitted(|actual| {
            assert_eq!(actual, demand);
            Err::<ScalarPolicy, _>(47)
        })),
        Err(MapAdmissionError::Refused(47))
    ));
    let mut outer = writer.checkpoint().unwrap();
    assert_eq!(without_allocations(|| outer.clear_demand()), Ok(demand));
    outer
        .try_clear_admitted(|actual| {
            assert_eq!(actual, demand);
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap();
    assert!(outer.is_empty());
    assert_eq!(outer.get_before(&17).unwrap() as *const usize, pointer);
    {
        let mut child = outer.checkpoint().unwrap();
        child
            .try_insert_admitted(999, 333, |_| Ok::<_, ()>(ScalarPolicy))
            .unwrap();
        child
            .try_clear_admitted(|_| Ok::<_, ()>(ScalarPolicy))
            .unwrap();
        child
            .try_insert_admitted(1000, 444, |_| Ok::<_, ()>(ScalarPolicy))
            .unwrap();
        without_allocations(|| child.apply());
    }
    assert_eq!(outer.get(&1000), Some(&444));
    without_allocations(|| drop(outer));
    assert_eq!(writer.inner.as_ref().get_root(), original_root);
    assert_eq!(writer.inner.as_ref().admitted_tracking(), original_tracking);
    assert_eq!(writer.get(&17).unwrap() as *const usize, pointer);
    assert_eq!(writer.len(), 96);
    assert!(writer.get(&1000).is_none());
    without_allocations(|| writer.commit());
    assert_eq!(map.read().len(), 96);
}

#[test]
fn admission_panic_poison_rejects_new_writer_and_clear_before_callbacks() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ =
            map.try_write_admitted(|_| -> Result<ScalarPolicy, ()> { panic!("admission panic") });
    }))
    .is_err());
    without_allocations(|| {
        assert!(matches!(
            map.try_write_admitted(|_| -> Result<ScalarPolicy, ()> {
                panic!("must reject poison")
            }),
            Err(MapAdmissionError::Poisoned)
        ));
        assert!(matches!(
            map.try_clear_admitted(|_| -> Result<ScalarPolicy, ()> {
                panic!("must reject poison")
            }),
            Err(MapAdmissionError::Poisoned)
        ));
    });
    assert!(map.read().is_empty());
}

#[derive(Default)]
struct ChargeState {
    drops: std::sync::atomic::AtomicUsize,
    panic_zero: std::sync::atomic::AtomicBool,
    panic_nonzero: std::sync::atomic::AtomicBool,
}
struct CleanupCharge {
    state: std::sync::Arc<ChargeState>,
    zero: bool,
}
impl Drop for CleanupCharge {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering;
        self.state.drops.fetch_add(1, Ordering::SeqCst);
        if (self.zero && self.state.panic_zero.swap(false, Ordering::SeqCst))
            || (!self.zero && self.state.panic_nonzero.swap(false, Ordering::SeqCst))
        {
            panic!("original tracking refund panic");
        }
    }
}
struct CleanupPolicy(std::sync::Arc<ChargeState>);
impl NodeFunding for CleanupPolicy {
    type Charge = CleanupCharge;
    fn take_node_charge(&mut self, layout: Layout) -> Self::Charge {
        CleanupCharge {
            state: self.0.clone(),
            zero: layout.size() == 0,
        }
    }
}
impl NodeCloning<usize, usize> for CleanupPolicy {
    fn clone_key(&mut self, key: &usize) -> usize {
        *key
    }
    fn clone_value(&mut self, value: &usize) -> usize {
        *value
    }
}
impl ClonePlanning<usize, usize> for CleanupPolicy {
    fn plan_key(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}

#[test]
fn applying_retaining_transfers_both_checkpoints_before_any_original_charge_drop() {
    use std::{
        panic::{catch_unwind, AssertUnwindSafe},
        sync::{atomic::Ordering, Arc},
    };
    let state = Arc::new(ChargeState::default());
    let first =
        BptreeMap::<usize, usize, Prepaid<CleanupPolicy>>::try_new_with_node_custody(|_| {
            Ok::<_, ()>(CleanupPolicy(state.clone()))
        })
        .unwrap();
    let second =
        BptreeMap::<usize, usize, Prepaid<CleanupPolicy>>::try_new_with_node_custody(|_| {
            Ok::<_, ()>(CleanupPolicy(state.clone()))
        })
        .unwrap();
    let mut first_writer = first
        .try_write_admitted(|_| Ok::<_, ()>(CleanupPolicy(state.clone())))
        .unwrap();
    let mut second_writer = second
        .try_write_admitted(|_| Ok::<_, ()>(CleanupPolicy(state.clone())))
        .unwrap();
    let mut first_checkpoint = first_writer.checkpoint().unwrap();
    let mut second_checkpoint = second_writer.checkpoint().unwrap();
    first_checkpoint
        .try_insert_admitted(1, 11, |_| Ok::<_, ()>(CleanupPolicy(state.clone())))
        .unwrap();
    second_checkpoint
        .try_insert_admitted(2, 22, |_| Ok::<_, ()>(CleanupPolicy(state.clone())))
        .unwrap();
    let before = state.drops.load(Ordering::SeqCst);
    state.panic_zero.store(true, Ordering::SeqCst);
    let retired = without_allocations(|| {
        (
            first_checkpoint.apply_retaining(),
            second_checkpoint.apply_retaining(),
        )
    });
    assert_eq!(state.drops.load(Ordering::SeqCst), before);
    assert_eq!(first_writer.get(&1), Some(&11));
    assert_eq!(second_writer.get(&2), Some(&22));
    assert!(catch_unwind(AssertUnwindSafe(|| drop(retired))).is_err());
    assert_eq!(state.drops.load(Ordering::SeqCst), before + 4);
    // The higher-level joint owner must remain failed after that cleanup panic.
    // Abort both complete private cursors; neither original map was published.
    drop(first_writer);
    drop(second_writer);
    assert!(first.read().is_empty());
    assert!(second.read().is_empty());
}

struct PanicOnFinish(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Drop for PanicOnFinish {
    fn drop(&mut self) {
        if self.0.swap(false, std::sync::atomic::Ordering::SeqCst) {
            panic!("unused clear funding destructor");
        }
    }
}
impl NodeFunding for PanicOnFinish {
    type Charge = Untracked;
    fn take_node_charge(&mut self, _: Layout) -> Untracked {
        Untracked
    }
}
impl NodeCloning<usize, usize> for PanicOnFinish {
    fn clone_key(&mut self, key: &usize) -> usize {
        *key
    }
    fn clone_value(&mut self, value: &usize) -> usize {
        *value
    }
}
impl ClonePlanning<usize, usize> for PanicOnFinish {
    fn plan_key(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}

#[test]
fn completed_clear_cannot_publish_after_caught_unused_funding_panic() {
    use std::{
        panic::{catch_unwind, AssertUnwindSafe},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };
    let panic = Arc::new(AtomicBool::new(false));
    let map = BptreeMap::<usize, usize, Prepaid<PanicOnFinish>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(PanicOnFinish(panic.clone()))
    })
    .unwrap();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(PanicOnFinish(panic.clone())))
        .unwrap();
    writer
        .try_insert_admitted(3, 33, |_| Ok::<_, ()>(PanicOnFinish(panic.clone())))
        .unwrap();
    writer.commit();
    let original = map.read();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(PanicOnFinish(panic.clone())))
        .unwrap();
    panic.store(true, Ordering::SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| {
        writer
            .try_clear_admitted(|_| Ok::<_, ()>(PanicOnFinish(panic.clone())))
            .unwrap();
    }))
    .is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| writer.len())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| writer.commit())).is_err());
    assert_eq!(original.get(&3), Some(&33));
    assert_eq!(map.read().get(&3), Some(&33));
}

#[test]
fn writer_and_clear_generation_exhaustion_refuse_before_admission_or_allocation() {
    let mut provider = Prepaid(Some(ScalarPolicy));
    let mut source = unsafe {
        SuperBlock::<usize, usize, Prepaid<ScalarPolicy>>::new_with_funding(&mut provider)
    };
    source.txid = (TXID_MASK >> TXID_SHF) - 1;
    let map = BptreeMap {
        inner: LinCowCell::new_charged(
            source,
            InitialCharges {
                root: Untracked,
                reader: Untracked,
            },
        ),
    };
    without_allocations(|| {
        assert!(matches!(
            map.try_write_admitted(|_| -> Result<ScalarPolicy, ()> {
                panic!("exhausted generation")
            }),
            Err(MapAdmissionError::Planning(PlanningError::Overflow))
        ));
        assert!(matches!(
            map.try_clear_admitted(|_| -> Result<ScalarPolicy, ()> {
                panic!("exhausted generation")
            }),
            Err(MapAdmissionError::Planning(PlanningError::Overflow))
        ));
    });
    assert!(map.read().is_empty());
    assert!(!map.is_poisoned());
}

#[test]
fn staged_pair_commit_retains_all_cleanup_until_both_original_locks_are_released() {
    use std::{
        panic::{catch_unwind, AssertUnwindSafe},
        sync::{atomic::Ordering, Arc},
    };
    let state = Arc::new(ChargeState::default());
    let first =
        BptreeMap::<usize, usize, Prepaid<CleanupPolicy>>::try_new_with_node_custody(|_| {
            Ok::<_, ()>(CleanupPolicy(state.clone()))
        })
        .unwrap();
    let second =
        BptreeMap::<usize, usize, Prepaid<CleanupPolicy>>::try_new_with_node_custody(|_| {
            Ok::<_, ()>(CleanupPolicy(state.clone()))
        })
        .unwrap();
    let mut first_writer = first
        .try_write_admitted(|_| Ok::<_, ()>(CleanupPolicy(state.clone())))
        .unwrap();
    let mut second_writer = second
        .try_write_admitted(|_| Ok::<_, ()>(CleanupPolicy(state.clone())))
        .unwrap();
    first_writer
        .try_insert_admitted(1, 11, |_| Ok::<_, ()>(CleanupPolicy(state.clone())))
        .unwrap();
    second_writer
        .try_insert_admitted(2, 22, |_| Ok::<_, ()>(CleanupPolicy(state.clone())))
        .unwrap();
    let before = state.drops.load(Ordering::SeqCst);
    state.panic_nonzero.store(true, Ordering::SeqCst);
    let (first_ready, second_ready) = without_allocations(|| {
        (
            first_writer.prepare_commit(),
            second_writer.prepare_commit(),
        )
    });
    assert_eq!(state.drops.load(Ordering::SeqCst), before);
    let (first_published, second_published) =
        without_allocations(|| (first_ready.publish(), second_ready.publish()));
    assert_eq!(state.drops.load(Ordering::SeqCst), before);
    without_allocations(|| {
        assert!(matches!(
            first.try_write_admitted(|_| -> Result<CleanupPolicy, ()> {
                panic!("published original writer remains held")
            }),
            Err(MapAdmissionError::Busy)
        ));
        assert!(matches!(
            second.try_write_admitted(|_| -> Result<CleanupPolicy, ()> {
                panic!("published original writer remains held")
            }),
            Err(MapAdmissionError::Busy)
        ));
    });
    let retirement =
        without_allocations(|| (first_published.release(), second_published.release()));
    assert_eq!(state.drops.load(Ordering::SeqCst), before);
    assert_eq!(first.read().get(&1), Some(&11));
    assert_eq!(second.read().get(&2), Some(&22));
    // The injected first-seen buffer refund runs only after both publications
    // and physical unlocks. Its panic cannot skip the second map publication.
    assert!(catch_unwind(AssertUnwindSafe(|| drop(retirement))).is_err());
    assert!(!first.is_poisoned());
    assert!(!second.is_poisoned());
    assert_eq!(first.read().get(&1), Some(&11));
    assert_eq!(second.read().get(&2), Some(&22));
    without_allocations(|| {
        assert!(matches!(
            first.try_write_admitted(|_| Err::<CleanupPolicy, _>(53)),
            Err(MapAdmissionError::Refused(53))
        ));
        assert!(matches!(
            second.try_write_admitted(|_| Err::<CleanupPolicy, _>(53)),
            Err(MapAdmissionError::Refused(53))
        ));
    });
}

#[test]
fn prepared_pair_abort_releases_original_owners_without_publishing_either_tree() {
    let first = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let second =
        BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap();
    let mut first_writer = first
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    let mut second_writer = second
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    first_writer
        .try_insert_admitted(1, 11, |_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    second_writer
        .try_insert_admitted(2, 22, |_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    let ready = without_allocations(|| {
        (
            first_writer.prepare_commit(),
            second_writer.prepare_commit(),
        )
    });
    without_allocations(|| drop(ready));
    assert!(first.read().is_empty());
    assert!(second.read().is_empty());
    assert!(!first.is_poisoned());
    assert!(!second.is_poisoned());
}

thread_local! {
    static REMOVAL_PLAN_COUNTS: std::cell::Cell<[usize; 3]> = const { std::cell::Cell::new([0; 3]) };
    static REMOVAL_REJECT_KEY: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

struct RemovalPlanningPolicy;
impl NodeFunding for RemovalPlanningPolicy {
    type Charge = Untracked;
    fn take_node_charge(&mut self, _: Layout) -> Untracked {
        Untracked
    }
}
impl NodeCloning<usize, usize> for RemovalPlanningPolicy {
    fn clone_key(&mut self, key: &usize) -> usize {
        REMOVAL_PLAN_COUNTS.with(|counts| {
            let mut next = counts.get();
            next[2] += 1;
            counts.set(next);
        });
        *key
    }
    fn clone_value(&mut self, value: &usize) -> usize {
        self.clone_key(value)
    }
}
impl ClonePlanning<usize, usize> for RemovalPlanningPolicy {
    fn plan_key(key: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        REMOVAL_PLAN_COUNTS.with(|counts| {
            let mut next = counts.get();
            next[0] += 1;
            counts.set(next);
        });
        if REMOVAL_REJECT_KEY.with(|rejected| rejected.get() == Some(*key)) {
            Err(PlanningError::UnsupportedPayload)
        } else {
            Ok(())
        }
    }
    fn plan_value(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        REMOVAL_PLAN_COUNTS.with(|counts| {
            let mut next = counts.get();
            next[1] += 1;
            counts.set(next);
        });
        Ok(())
    }
}

fn removal_planning_map() -> BptreeMap<usize, usize, Prepaid<RemovalPlanningPolicy>> {
    REMOVAL_REJECT_KEY.with(|rejected| rejected.set(None));
    let map = BptreeMap::try_new_with_node_custody(|_| Ok::<_, ()>(RemovalPlanningPolicy)).unwrap();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(RemovalPlanningPolicy))
        .unwrap();
    for key in 0..1024 {
        writer
            .try_insert_admitted(key, key * 3, |_| Ok::<_, ()>(RemovalPlanningPolicy))
            .unwrap();
    }
    writer.commit();
    map
}

#[test]
fn removal_planning_rejects_descendant_minimum_before_admission_or_mutation() {
    let map = removal_planning_map();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(RemovalPlanningPolicy))
        .unwrap();
    let original_root = writer.inner.as_ref().get_root();
    // The left sibling is a branch. Its minimum belongs to a descendant leaf,
    // not to either branch's stored separator prefix or the requested path.
    let root = unsafe { &*original_root.cast::<Branch<usize, usize>>() };
    let sibling = root.get_idx_unchecked(0);
    assert!(!unsafe { &*sibling }.is_leaf());
    let sibling = unsafe { &*sibling.cast::<Branch<usize, usize>>() };
    let rejected = unsafe { *Node::min_raw(root.get_idx_unchecked(0)) };
    let target = *root.key_at(0);
    assert!(target > rejected);
    assert!((0..root.count()).all(|index| *root.key_at(index) != rejected));
    assert!((0..sibling.count()).all(|index| *sibling.key_at(index) != rejected));
    let original_tracking = writer.inner.as_ref().admitted_tracking();
    REMOVAL_PLAN_COUNTS.with(|counts| counts.set([0; 3]));
    REMOVAL_REJECT_KEY.with(|key| key.set(Some(rejected)));
    without_allocations(|| {
        assert_eq!(
            writer.removal_demand(&target),
            Err(PlanningError::UnsupportedPayload)
        );
        assert!(matches!(
            writer.try_remove_admitted(&target, |_| -> Result<RemovalPlanningPolicy, ()> {
                panic!("unsupported descendant copy must precede admission")
            }),
            Err(MapAdmissionError::Planning(
                PlanningError::UnsupportedPayload
            ))
        ));
        // Absence remains a true no-op even while the copy policy refuses.
        REMOVAL_PLAN_COUNTS.with(|counts| counts.set([0; 3]));
        assert_eq!(
            writer.removal_demand(&usize::MAX),
            Ok(AllocationDemand::new())
        );
        assert_eq!(
            writer
                .try_remove_admitted(&usize::MAX, |_| -> Result<RemovalPlanningPolicy, ()> {
                    panic!("absent removal must not request admission")
                })
                .unwrap(),
            None
        );
        assert_eq!(REMOVAL_PLAN_COUNTS.with(std::cell::Cell::get), [0; 3]);
    });
    REMOVAL_REJECT_KEY.with(|key| key.set(None));
    assert_eq!(writer.inner.as_ref().get_root(), original_root);
    assert_eq!(writer.inner.as_ref().admitted_tracking(), original_tracking);
    assert_eq!(writer.len(), 1024);
    assert_eq!(writer.get(&target), Some(&(target * 3)));
    assert_eq!(writer.get(&rejected), Some(&(rejected * 3)));
    assert_eq!(
        writer
            .try_remove_admitted(&target, |_| Ok::<_, ()>(RemovalPlanningPolicy))
            .unwrap(),
        Some(target * 3)
    );
    drop(writer);
    assert_eq!(map.read().len(), 1024);
    assert_eq!(map.read().get(&target), Some(&(target * 3)));
    assert!(!map.is_poisoned());
}

#[test]
fn removal_planning_payload_visits_are_height_bounded_and_match_exact_refusal() {
    use crate::internals::bptree::node::L_CAPACITY;
    let map = removal_planning_map();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(RemovalPlanningPolicy))
        .unwrap();
    for target in [0, 511, 1023] {
        let original_root = writer.inner.as_ref().get_root();
        let original_tracking = writer.inner.as_ref().admitted_tracking();
        let mut node = original_root;
        let mut branches = 0;
        while !unsafe { &*node }.is_leaf() {
            branches += 1;
            let branch = unsafe { &*node.cast::<Branch<usize, usize>>() };
            node = branch.get_idx_unchecked(branch.locate_node(&target));
        }
        assert!(branches >= 2);
        REMOVAL_PLAN_COUNTS.with(|counts| counts.set([0; 3]));
        let demand = without_allocations(|| writer.removal_demand(&target).unwrap());
        let observed = REMOVAL_PLAN_COUNTS.with(std::cell::Cell::get);
        // This bounds policy calls, not the separate min_raw pointer descent.
        // Every level examines its path node and at most one sibling; no full
        // tree traversal or payload copy is needed for an admission decision.
        assert!(observed[0] <= (2 * branches + 1) * (2 * L_CAPACITY + 1));
        assert!(observed[0] < writer.len() / 4);
        assert!(observed[1] <= 2 * L_CAPACITY);
        assert_eq!(observed[2], 0);
        REMOVAL_PLAN_COUNTS.with(|counts| counts.set([0; 3]));
        let refused = without_allocations(|| {
            writer.try_remove_admitted(&target, |actual| {
                assert_eq!(actual, demand);
                Err::<RemovalPlanningPolicy, _>(71)
            })
        });
        assert!(matches!(refused, Err(MapAdmissionError::Refused(71))));
        assert_eq!(REMOVAL_PLAN_COUNTS.with(std::cell::Cell::get), observed);
        assert_eq!(writer.inner.as_ref().get_root(), original_root);
        assert_eq!(writer.inner.as_ref().admitted_tracking(), original_tracking);
        assert_eq!(writer.get(&target), Some(&(target * 3)));
    }
}

#[test]
fn original_owned_prepaid_edits_replan_and_refuse_before_copies_without_a_writer() {
    let map =
        BptreeMap::<usize, Box<[u8]>, Prepaid<SizedPayloadPolicy>>::try_new_with_node_custody(
            |_| Ok::<_, ()>(SizedPayloadPolicy),
        )
        .unwrap();
    let mut owned = map
        .try_write_admitted(|_| Ok::<_, ()>(SizedPayloadPolicy))
        .unwrap()
        .detach();
    let cursor = owned.inner.as_ref() as *const _;
    let held = map
        .try_write_admitted(|_| Ok::<_, ()>(SizedPayloadPolicy))
        .unwrap();
    let input = vec![7; 131].into_boxed_slice();
    let pointer = input.as_ptr();
    let demand = without_allocations(|| owned.insertion_demand(&1).unwrap());
    let refused = without_allocations(|| {
        owned.try_insert_admitted(1, input, |actual| {
            assert_eq!(actual, demand);
            Err::<SizedPayloadPolicy, _>(73)
        })
    });
    let Err(((1, input), MapAdmissionError::Refused(73))) = refused else {
        panic!("original input refusal")
    };
    assert_eq!(input.as_ptr(), pointer);
    assert_eq!(owned.inner.as_ref() as *const _, cursor);
    assert!(owned.get(&1).is_none());
    owned
        .try_insert_admitted(1, input, |actual| {
            assert_eq!(actual, demand);
            Ok::<_, ()>(SizedPayloadPolicy)
        })
        .unwrap();
    assert_eq!(owned.get(&1).unwrap().as_ptr(), pointer);
    assert!(held.get(&1).is_none());
    let removal = without_allocations(|| owned.removal_demand(&1).unwrap());
    assert!(matches!(
        without_allocations(|| owned.try_remove_admitted(&1, |actual| {
            assert_eq!(actual, removal);
            Err::<SizedPayloadPolicy, _>(79)
        })),
        Err(MapAdmissionError::Refused(79))
    ));
    assert_eq!(owned.get(&1).unwrap().as_ptr(), pointer);
    let removed = owned
        .try_remove_admitted(&1, |actual| {
            assert_eq!(actual, removal);
            Ok::<_, ()>(SizedPayloadPolicy)
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        removed.as_ptr(),
        pointer,
        "original private value returns by move"
    );
    assert!(held.is_empty());
    drop(held);
    map.try_write_owned(owned)
        .unwrap_or_else(|_| panic!("original unchanged base"))
        .commit();
    assert!(map.read().is_empty());
}

#[test]
fn original_owned_copy_panic_blocks_read_edit_and_reacquisition_without_poisoning_map() {
    use std::{
        panic::{catch_unwind, AssertUnwindSafe},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };
    let drops = Arc::new(AtomicUsize::new(0));
    let map =
        BptreeMap::<usize, CountedValue, Prepaid<NoValueCopies>>::try_new_with_node_custody(|_| {
            Ok::<_, ()>(NoValueCopies)
        })
        .unwrap();
    let (owned, _) = map
        .try_insert_admitted(
            1,
            CountedValue {
                value: Box::new(7),
                drops: drops.clone(),
            },
            |_| Ok::<_, ()>(NoValueCopies),
        )
        .unwrap_or_else(|_| panic!("first input needs no value copy"));
    map.try_write_owned(owned)
        .unwrap_or_else(|_| panic!("first publication"))
        .commit();
    let original = map.read();
    let mut owned = map
        .try_write_admitted(|_| Ok::<_, ()>(NoValueCopies))
        .unwrap()
        .detach();
    let held = map
        .try_write_admitted(|_| Ok::<_, ()>(NoValueCopies))
        .unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = owned.try_insert_admitted(
            2,
            CountedValue {
                value: Box::new(11),
                drops: drops.clone(),
            },
            |_| Ok::<_, ()>(NoValueCopies),
        );
    }))
    .is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| owned.get(&1))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| owned.insertion_demand(&3))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(
        || owned.try_remove_admitted(&1, |_| Ok::<_, ()>(NoValueCopies))
    ))
    .is_err());
    // Reject logical failure before trying the still-held physical writer.
    // Returning Busy with the failed owner would not satisfy this assertion.
    assert!(catch_unwind(AssertUnwindSafe(|| map.try_write_owned(owned))).is_err());
    assert_eq!(*original.get(&1).unwrap().value, 7);
    assert_eq!(*held.get(&1).unwrap().value, 7);
    assert!(original.get(&2).is_none());
    assert!(!map.is_poisoned());
    drop(held);
    assert!(!map.is_poisoned());
    drop(original);
    drop(map);
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

struct OwnedAllocationCharge {
    _original: Option<crate::internals::bptree::node::allocation_tests::Charge>,
}
struct OwnedAllocationPolicy {
    funding:
        std::rc::Rc<std::cell::RefCell<crate::internals::bptree::node::allocation_tests::Prepaid>>,
    observe: bool,
    first_only: bool,
}
impl NodeFunding for OwnedAllocationPolicy {
    type Charge = OwnedAllocationCharge;
    fn take_node_charge(&mut self, layout: Layout) -> Self::Charge {
        let observe = self.observe;
        if self.first_only {
            self.observe = false;
        }
        OwnedAllocationCharge {
            _original: observe.then(|| self.funding.borrow_mut().take_allocation_charge(layout)),
        }
    }
}
impl NodeCloning<usize, usize> for OwnedAllocationPolicy {
    fn clone_key(&mut self, key: &usize) -> usize {
        *key
    }
    fn clone_value(&mut self, value: &usize) -> usize {
        *value
    }
}
impl ClonePlanning<usize, usize> for OwnedAllocationPolicy {
    fn plan_key(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}

#[test]
fn original_owned_stale_refusal_retains_actual_private_allocations_until_abort() {
    use crate::internals::bptree::node::allocation_tests::{all_refunded, prepaid, record};
    use std::{cell::RefCell, rc::Rc};
    let funding = Rc::new(RefCell::new(prepaid()));
    let policy = |observe, first_only| OwnedAllocationPolicy {
        funding: funding.clone(),
        observe,
        first_only,
    };
    // This observer charges actual nodes and edit tracking buffers. Initial
    // control/shell lifetimes are independently covered by linear-cell tests.
    let map = BptreeMap::<usize, usize, Prepaid<OwnedAllocationPolicy>>::try_new_with_node_custody(
        |_| Ok::<_, ()>(policy(true, true)),
    )
    .unwrap();
    let mut seed = map
        .try_write_admitted(|_| Ok::<_, ()>(policy(false, false)))
        .unwrap();
    seed.try_insert_admitted(0, 7, |_| Ok::<_, ()>(policy(true, false)))
        .unwrap();
    seed.commit();
    let original = map.read();
    let mut owned = map
        .try_write_admitted(|_| Ok::<_, ()>(policy(false, false)))
        .unwrap()
        .detach();
    let cursor = owned.inner.as_ref() as *const _;
    let begin = funding.as_ref().borrow().next;
    let mut held = map
        .try_write_admitted(|_| Ok::<_, ()>(policy(false, false)))
        .unwrap();
    owned
        .try_insert_admitted(1, 11, |_| Ok::<_, ()>(policy(true, false)))
        .unwrap();
    // Identify only this original private edit's live allocations before the
    // unrelated edit/publication; records contain actual pointers and layouts.
    let end = funding.as_ref().borrow().next;
    let own_live: Vec<_> = (begin..end).filter(|&id| !record(id).freed).collect();
    held.try_insert_admitted(9, 99, |_| Ok::<_, ()>(policy(true, false)))
        .unwrap();
    held.commit();
    assert!(!own_live.is_empty());
    let (owned, error) = without_allocations(|| {
        map.try_write_owned(owned)
            .err()
            .expect("stale original generation")
    });
    assert_eq!(error, OwnedWriteError::Changed);
    assert_eq!(owned.inner.as_ref() as *const _, cursor);
    assert_eq!(owned.get(&1), Some(&11));
    assert_eq!(original.get(&1), None);
    assert_eq!(map.read().get(&9), Some(&99));
    for &id in &own_live {
        assert!(!record(id).freed && !record(id).refunded);
    }
    drop(owned);
    assert!(own_live
        .iter()
        .all(|&id| record(id).freed && record(id).refunded));
    drop(original);
    drop(map);
    all_refunded(&funding.as_ref().borrow());
}

#[test]
fn prepaid_private_copy_updates_append_and_replacement_without_allocating_or_replanning() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let mut seed = map
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    for key in 0..128 {
        seed.try_insert_admitted(key, key * 3, |_| Ok::<_, ()>(ScalarPolicy))
            .unwrap();
    }
    seed.commit();
    for (key, previous) in [(128, None), (128, Some(777))] {
        let old = map.read();
        let (mut successor, replaced) = map
            .try_insert_admitted(key, 0, |_| Ok::<_, ()>(ScalarPolicy))
            .unwrap_or_else(|_| panic!("complete original successor admission"));
        assert_eq!(replaced, previous);
        let cursor = successor.inner.as_ref() as *const _;
        let root = successor.inner.as_ref().get_root();
        let tracking = successor.inner.as_ref().admitted_tracking();
        let predecessor = successor.predecessor().retain();
        let held = map
            .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
            .unwrap();
        without_allocations(|| {
            assert_eq!(successor.try_update_private(&key, 777), Ok(0));
            assert_eq!(successor.try_update_private(&(key + 1), 999), Err(999));
            assert_eq!(successor.inner.as_ref() as *const _, cursor);
            assert_eq!(successor.inner.as_ref().get_root(), root);
            assert_eq!(successor.inner.as_ref().admitted_tracking(), tracking);
            assert!(predecessor.matches(&successor.predecessor()));
            assert_eq!(old.get(&key).copied(), previous);
            assert_eq!(held.get(&key).copied(), previous);
        });
        drop(held);
        map.try_write_owned(successor)
            .unwrap_or_else(|_| panic!("publish same prepaid successor"))
            .commit();
        assert_eq!(map.read().get(&key), Some(&777));
        assert_eq!(old.get(&key).copied(), previous);
    }
}

#[test]
fn prepaid_private_copy_refuses_shared_leaf_without_allocating_or_mutating_it() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let (seed, _) = map
        .try_insert_admitted(1, 7, |_| Ok::<_, ()>(ScalarPolicy))
        .unwrap_or_else(|_| panic!("seed"));
    map.try_write_owned(seed)
        .unwrap_or_else(|_| panic!("publish"))
        .commit();
    let old = map.read();
    let mut unchanged = map
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap()
        .detach();
    without_allocations(|| {
        assert_eq!(unchanged.try_update_private(&1, 11), Err(11));
        assert_eq!(unchanged.try_update_private(&9, 19), Err(19));
        assert_eq!(old.get(&1), Some(&7));
        assert_eq!(unchanged.get(&1), Some(&7));
    });
    map.try_write_owned(unchanged)
        .unwrap_or_else(|_| panic!("a normal refusal preserves usable owner"))
        .commit();
    assert_eq!(map.read().get(&1), Some(&7));
}

#[test]
fn prepaid_private_copy_retains_actual_allocation_custody_through_stale_abort() {
    use crate::internals::bptree::node::allocation_tests::{all_refunded, prepaid, record};
    use std::{cell::RefCell, rc::Rc};
    let funding = Rc::new(RefCell::new(prepaid()));
    let policy = |observe, first_only| OwnedAllocationPolicy {
        funding: funding.clone(),
        observe,
        first_only,
    };
    let map = BptreeMap::<usize, usize, Prepaid<OwnedAllocationPolicy>>::try_new_with_node_custody(
        |_| Ok::<_, ()>(policy(true, true)),
    )
    .unwrap();
    let mut owner = map
        .try_write_admitted(|_| Ok::<_, ()>(policy(false, false)))
        .unwrap()
        .detach();
    let begin = funding.as_ref().borrow().next;
    owner
        .try_insert_admitted(0, 0, |_| Ok::<_, ()>(policy(true, false)))
        .unwrap();
    let end = funding.as_ref().borrow().next;
    let original = owner.inner.as_ref() as *const _;
    let old = map.read();
    let mut winner = map
        .try_write_admitted(|_| Ok::<_, ()>(policy(false, false)))
        .unwrap();
    without_allocations(|| assert_eq!(owner.try_update_private(&0, 17), Ok(0)));
    winner
        .try_insert_admitted(0, 19, |_| Ok::<_, ()>(policy(true, false)))
        .unwrap();
    winner.commit();
    without_allocations(|| assert_eq!(owner.try_update_private(&0, 23), Ok(17)));
    let (owner, error) = without_allocations(|| map.try_write_owned(owner).err().unwrap());
    assert_eq!(error, OwnedWriteError::Changed);
    assert_eq!(owner.inner.as_ref() as *const _, original);
    assert_eq!(owner.get(&0), Some(&23));
    assert!(old.is_empty());
    assert_eq!(map.read().get(&0), Some(&19));
    for id in begin..end {
        assert!(
            !record(id).refunded,
            "private custody survives stale refusal"
        );
    }
    drop(owner);
    for id in begin..end {
        assert!(record(id).freed && record(id).refunded);
    }
    drop(old);
    drop(map);
    all_refunded(&funding.as_ref().borrow());
}

thread_local! {
    static PRIVATE_UPDATE_COMPARE_PANIC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct PrivateUpdateKey(usize);
impl Ord for PrivateUpdateKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        PRIVATE_UPDATE_COMPARE_PANIC
            .with(|panic| assert!(!panic.get(), "private comparison unwind"));
        self.0.cmp(&other.0)
    }
}
impl PartialOrd for PrivateUpdateKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl NodeCloning<PrivateUpdateKey, usize> for ScalarPolicy {
    fn clone_key(&mut self, key: &PrivateUpdateKey) -> PrivateUpdateKey {
        *key
    }
    fn clone_value(&mut self, value: &usize) -> usize {
        *value
    }
}
impl ClonePlanning<PrivateUpdateKey, usize> for ScalarPolicy {
    fn plan_key(_: &PrivateUpdateKey, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}
#[test]
fn prepaid_private_copy_comparison_unwind_requires_original_owner_abort() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let map =
        BptreeMap::<PrivateUpdateKey, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(
            |_| Ok::<_, ()>(ScalarPolicy),
        )
        .unwrap();
    let (mut owner, _) = map
        .try_insert_admitted(PrivateUpdateKey(1), 7, |_| Ok::<_, ()>(ScalarPolicy))
        .unwrap_or_else(|_| panic!("admit original entry"));
    PRIVATE_UPDATE_COMPARE_PANIC.with(|panic| panic.set(true));
    let result = catch_unwind(AssertUnwindSafe(|| {
        owner.try_update_private(&PrivateUpdateKey(1), 11)
    }));
    PRIVATE_UPDATE_COMPARE_PANIC.with(|panic| panic.set(false));
    assert!(result.is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| owner.get(&PrivateUpdateKey(1)))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| map.try_write_owned(owner))).is_err());
    assert!(map.read().is_empty());
    assert!(!map.is_poisoned());
}

fn observed_current_footprint<P: ClonePlanning<usize, usize>>(
    map: &BptreeMap<usize, usize, Prepaid<P>>,
) -> AllocationDemand {
    // Independent test oracle: production must never traverse this whole tree.
    let view = map.read();
    let mut expected = AllocationDemand::new();
    expected
        .add_layout(MapCell::<usize, usize, Prepaid<P>>::initial_allocation_layouts().root)
        .unwrap();
    expected
        .add_layout(MapCell::<usize, usize, Prepaid<P>>::reader_allocation_layout())
        .unwrap();
    unsafe {
        Node::visit_tree(view.inner.as_ref().get_root(), |node| {
            let layout = if (&*node).is_leaf() {
                Layout::new::<CachePadded<Leaf<usize, usize, P::Charge>>>()
            } else {
                Layout::new::<CachePadded<Branch<usize, usize, P::Charge>>>()
            };
            expected.add_layout(layout).ok()
        })
    }
    .unwrap();
    expected
}

fn checked_current_footprint<P: ClonePlanning<usize, usize>>(
    map: &BptreeMap<usize, usize, Prepaid<P>>,
) -> (AllocationDemand, AllocationDemand) {
    let expected = observed_current_footprint(map);
    let old = map.read();
    let original = old.inner.as_ref().get_root();
    let result = without_allocations(|| {
        map.try_insert_admitted_with_footprint(usize::MAX, 0, |existing, additional| {
            assert_eq!(existing, expected);
            Err::<P, _>((existing, additional))
        })
    });
    assert_eq!(old.inner.as_ref().get_root(), original);
    match result {
        Err(((usize::MAX, 0), MapAdmissionError::Refused(demands))) => demands,
        _ => panic!("footprint callback must refuse before allocation"),
    }
}

#[test]
fn prepaid_current_footprint_tracks_original_split_merge_overwrite_and_clear_nodes() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let initial = checked_current_footprint(&map).0;
    assert_eq!(initial.allocations(), 3);
    for key in 0..192 {
        let expected = observed_current_footprint(&map);
        let (owner, _) = map
            .try_insert_admitted_with_footprint(key, key, |existing, _| {
                assert_eq!(existing, expected);
                Ok::<_, ()>(ScalarPolicy)
            })
            .unwrap_or_else(|_| panic!("funded growth"));
        map.try_write_owned(owner)
            .unwrap_or_else(|_| panic!("original generation"))
            .commit();
    }
    let grown = checked_current_footprint(&map).0;
    assert!(grown.allocations() > initial.allocations());
    let retained = map.read();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    for key in 0..192 {
        assert_eq!(
            writer
                .try_insert_admitted(key, key + 1, |_| Ok::<_, ()>(ScalarPolicy))
                .unwrap(),
            Some(key)
        );
    }
    writer.commit();
    assert_eq!(checked_current_footprint(&map).0, grown);
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    for key in (0..192).step_by(2) {
        assert_eq!(
            writer
                .try_remove_admitted(&key, |_| Ok::<_, ()>(ScalarPolicy))
                .unwrap(),
            Some(key + 1)
        );
    }
    writer.commit();
    checked_current_footprint(&map);
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    for key in (1..192).step_by(2).rev() {
        writer
            .try_remove_admitted(&key, |_| Ok::<_, ()>(ScalarPolicy))
            .unwrap();
    }
    writer.commit();
    assert_eq!(checked_current_footprint(&map).0, initial);
    assert_eq!(
        retained.len(),
        192,
        "old readers do not enter the resident floor"
    );
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    for key in 0..192 {
        writer
            .try_insert_admitted(key, key, |_| Ok::<_, ()>(ScalarPolicy))
            .unwrap();
    }
    // Newly created nodes also enter last_seen. Both original lists must cancel
    // them in the reachable-node count, although old readers retain their free.
    writer
        .try_clear_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    writer.commit();
    assert_eq!(checked_current_footprint(&map).0, initial);
}

#[test]
fn prepaid_current_footprint_preserves_parent_through_checkpoint_and_publication_abort() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    for key in 0..128 {
        writer
            .try_insert_admitted(key, key, |_| Ok::<_, ()>(ScalarPolicy))
            .unwrap();
    }
    writer.commit();
    let baseline = checked_current_footprint(&map).0;
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    {
        let mut outer = writer.checkpoint().unwrap();
        for key in 128..256 {
            outer
                .try_insert_admitted(key, key, |_| Ok::<_, ()>(ScalarPolicy))
                .unwrap();
        }
        let mut child = outer.checkpoint().unwrap();
        for key in 0..128 {
            child
                .try_remove_admitted(&key, |_| Ok::<_, ()>(ScalarPolicy))
                .unwrap();
        }
        child.apply();
        // Child applies, but the original outer tracking cuts still roll back.
    }
    writer.commit();
    assert_eq!(checked_current_footprint(&map).0, baseline);
    let mut writer = map
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    {
        let mut outer = writer.checkpoint().unwrap();
        let mut child = outer.checkpoint().unwrap();
        child
            .try_clear_admitted(|_| Ok::<_, ()>(ScalarPolicy))
            .unwrap();
        child
            .try_insert_admitted(1000, 77, |_| Ok::<_, ()>(ScalarPolicy))
            .unwrap();
        child.apply();
        outer.apply();
    }
    let prepared = writer
        .try_prepare_commit()
        .unwrap_or_else(|_| panic!("prepare original"));
    let original = prepared.abort().detach();
    assert_eq!(checked_current_footprint(&map).0, baseline);
    map.try_write_owned(original)
        .unwrap_or_else(|_| panic!("retry exact owner"))
        .commit();
    let current = checked_current_footprint(&map).0;
    assert_eq!(current.allocations(), 3);
    assert_eq!(map.read().get(&1000), Some(&77));
}

#[test]
fn prepaid_current_footprint_distinguishes_resident_floor_from_refundable_old_custody() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let (existing, additional) = checked_current_footprint(&map);
    let mut complete = existing;
    complete.add_demand(additional).unwrap();
    // A pool smaller than the necessary resident+reservation sum cannot be
    // repaired by freeing old generations: this map has none to release.
    let limit = complete.bytes() - 1;
    let refused = without_allocations(|| {
        map.try_insert_admitted_with_footprint(1, 7, |current, next| {
            let mut total = current;
            total.add_demand(next).unwrap();
            assert!(total.bytes() > limit);
            Err::<ScalarPolicy, _>(total.bytes())
        })
    });
    assert!(matches!(
        refused,
        Err(((1, 7), MapAdmissionError::Refused(_)))
    ));
    assert_eq!(checked_current_footprint(&map).0, existing);
    let old = map.read();
    let (owner, _) = map
        .try_insert_admitted_with_footprint(1, 7, |current, next| {
            assert_eq!(current, existing);
            assert_eq!(next, additional);
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap_or_else(|_| panic!("full checked capacity"));
    map.try_write_owned(owner)
        .unwrap_or_else(|_| panic!("same owner"))
        .commit();
    let with_old = checked_current_footprint(&map).0;
    drop(old);
    assert_eq!(checked_current_footprint(&map).0, with_old);
    assert_eq!(with_old.allocations(), 3);
}

#[test]
fn prepaid_reader_predecessor_retains_original_generation_without_allocating_and_rejects_aba() {
    let map = BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
        Ok::<_, ()>(ScalarPolicy)
    })
    .unwrap();
    let foreign =
        BptreeMap::<usize, usize, Prepaid<ScalarPolicy>>::try_new_with_node_custody(|_| {
            Ok::<_, ()>(ScalarPolicy)
        })
        .unwrap();
    let read = map.read();
    let foreign_read = foreign.read();
    let original = without_allocations(|| read.predecessor().retain());
    let first = map
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    without_allocations(|| {
        assert!(original.matches(&first.predecessor()));
        assert!(read.predecessor().same_predecessor(&first.predecessor()));
        assert!(!original.matches(&foreign_read.predecessor()));
    });
    let detached = first.detach();
    assert!(original.matches(&detached.predecessor()));
    drop(detached);
    // Publish equal contents: the old pinned generation must not match the
    // new source despite identical length and values.
    map.try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap()
        .commit();
    let current = map.read();
    let successor = map
        .try_write_admitted(|_| Ok::<_, ()>(ScalarPolicy))
        .unwrap();
    without_allocations(|| {
        assert_eq!(read.len(), current.len());
        assert!(original.matches(&read.predecessor()));
        assert!(!original.matches(&current.predecessor()));
        assert!(!original.matches(&successor.predecessor()));
        assert!(current
            .predecessor()
            .same_predecessor(&successor.predecessor()));
    });
    drop(successor);
    drop(read);
    // The retained original cut remains distinct after its view is gone.
    assert!(!original.matches(&current.predecessor()));
}

mod writer_start {
    //! No-edit writer preflight, original tree custody and provider cleanup refusal.

    use super::super::*;
    use crate::internals::bptree::node::allocation_tests::without_allocations;
    use crate::internals::bptree::node::{TXID_MASK, TXID_SHF};
    use std::cell::Cell;
    use std::panic::{catch_unwind, AssertUnwindSafe};

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
        assert!(matches!(busy, Err(MapAdmissionError::Busy)));
        drop(writer);
        let refused = without_allocations(|| {
            map.try_write_admitted(|demand| {
                assert_eq!(demand.allocations(), 2);
                Err::<Policy, _>(19)
            })
        });
        assert!(matches!(refused, Err(MapAdmissionError::Refused(19))));
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
            Err(MapAdmissionError::Planning(PlanningError::Overflow))
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
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _writer = map.try_write_admitted(|_| Ok::<_, ()>(Policy { panic_drop: true }));
        }))
        .is_err());
        assert!(map.is_poisoned());
        assert_eq!(map.read().inner.as_ref().get_root(), root);
        assert_eq!(map.read().get_txid(), txid);
        assert_eq!(map.read().get(&0), Some(&0));
        let refused =
            without_allocations(|| map.try_write_admitted::<()>(|_| panic!("poisoned admission")));
        assert!(matches!(refused, Err(MapAdmissionError::Poisoned)));
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
