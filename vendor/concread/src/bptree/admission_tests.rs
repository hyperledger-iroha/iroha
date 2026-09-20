//! Checked planning and original input refusal before physical construction.

use super::*;
use crate::internals::bptree::node::allocation_tests::without_allocations;

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
            assert_eq!(demand.allocations(), 2);
            Err::<ScalarPolicy, _>(17)
        })
    });
    assert!(matches!(result, Err(17)));
}
