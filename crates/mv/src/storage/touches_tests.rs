//! Actual finite key/array owners for the original transaction touch metadata.

use super::*;
use crate::allocation::{AllocationBudget, AllocationReservation, without_allocations};
use std::{
    cmp::Ordering,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
};

#[derive(Default)]
struct Stats {
    copies: AtomicUsize,
    comparisons: AtomicUsize,
    drops: AtomicUsize,
    panic_copy: AtomicBool,
    unsupported: AtomicBool,
    panic_drop: AtomicUsize,
}
impl Stats {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            panic_drop: AtomicUsize::new(usize::MAX),
            ..Self::default()
        })
    }
}
struct Payload {
    value: Option<Box<usize>>,
    _charge: AllocationCharge,
    stats: Arc<Stats>,
}
impl Payload {
    fn input(value: usize, budget: &AllocationBudget, stats: &Arc<Stats>) -> Self {
        let mut reservation = budget.try_reserve(Layout::new::<usize>()).unwrap();
        let charge = reservation.try_split(Layout::new::<usize>()).unwrap();
        Self {
            value: Some(Box::new(value)),
            _charge: charge,
            stats: Arc::clone(stats),
        }
    }
    fn number(&self) -> usize {
        **self.value.as_ref().unwrap()
    }
    fn pointer(&self) -> usize {
        self.value.as_deref().unwrap() as *const usize as usize
    }
}
impl Clone for Payload {
    fn clone(&self) -> Self {
        panic!("touch keys must use the prepaid policy, never ordinary Clone")
    }
}
impl std::fmt::Debug for Payload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.number().fmt(f)
    }
}
impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        self.number() == other.number()
    }
}
impl Eq for Payload {}
impl PartialOrd for Payload {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Payload {
    fn cmp(&self, other: &Self) -> Ordering {
        self.stats.comparisons.fetch_add(1, SeqCst);
        self.number().cmp(&other.number())
    }
}
impl Drop for Payload {
    fn drop(&mut self) {
        let value = self.number();
        self.stats.drops.fetch_add(1, SeqCst);
        // Actual nested storage goes first, even for the selected destructor
        // panic. Its concrete charge then drops through normal field drop glue.
        drop(self.value.take());
        if self
            .stats
            .panic_drop
            .compare_exchange(value, usize::MAX, SeqCst, SeqCst)
            .is_ok()
        {
            panic!("selected funded touch-key destructor failed");
        }
    }
}
struct Policy {
    reservation: AllocationReservation,
    stats: Arc<Stats>,
}
impl NodeFunding for Policy {
    type Charge = AllocationCharge;
    fn take_node_charge(&mut self, layout: Layout) -> Self::Charge {
        self.reservation
            .try_split(layout)
            .expect("same original complete touch admission")
    }
}
impl NodeCloning<Payload, usize> for Policy {
    fn clone_key(&mut self, key: &Payload) -> Payload {
        self.stats.copies.fetch_add(1, SeqCst);
        let charge = self.take_node_charge(Layout::new::<usize>());
        let copy = Payload {
            value: Some(Box::new(key.number())),
            _charge: charge,
            stats: Arc::clone(&self.stats),
        };
        if self.stats.panic_copy.swap(false, SeqCst) {
            panic!("funded key copy failed after allocation");
        }
        copy
    }
    fn clone_value(&mut self, value: &usize) -> usize {
        *value
    }
}
impl ClonePlanning<Payload, usize> for Policy {
    fn plan_key(key: &Payload, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        if key.stats.unsupported.load(SeqCst) {
            return Err(PlanningError::UnsupportedPayload);
        }
        demand.add_layout(Layout::new::<usize>())
    }
    fn plan_value(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}
fn prepare<'a>(
    plan: TouchPlan<'a, '_, Payload>,
    budget: &AllocationBudget,
    stats: &Arc<Stats>,
) -> PreparedTouch<'a, Payload> {
    let mut provider = Policy {
        reservation: budget.try_reserve_bytes(plan.demand().bytes()).unwrap(),
        stats: Arc::clone(stats),
    };
    let prepared = plan.prepare::<usize, Policy>(&mut provider);
    assert_eq!(provider.reservation.remaining_bytes(), 0);
    prepared
}
fn insert(
    set: &mut SortedTouches<Payload>,
    key: &Payload,
    budget: &AllocationBudget,
    stats: &Arc<Stats>,
) {
    let plan = without_allocations(|| {
        set.plan::<usize, Policy>(key, AllocationDemand::new())
            .unwrap()
    });
    let prepared = prepare(plan, budget, stats);
    let comparisons = stats.comparisons.load(SeqCst);
    let retired = without_allocations(|| prepared.install());
    assert_eq!(stats.comparisons.load(SeqCst), comparisons);
    without_allocations(|| drop(retired));
}
fn identity(set: &SortedTouches<Payload>) -> (usize, usize, usize) {
    set.buffer.as_ref().map_or((0, 0, 0), |buffer| {
        (
            buffer.entries.as_ptr() as usize,
            buffer.initialized,
            buffer.entries.len(),
        )
    })
}

#[test]
fn touch_sorted_unique_growth_moves_original_key_allocations_without_copying() {
    let budget = AllocationBudget::new(64 * 1024);
    let stats = Stats::new();
    let keys = [30, 10, 50, 20, 40].map(|value| Payload::input(value, &budget, &stats));
    let mut set = without_allocations(SortedTouches::new);
    assert_eq!(set.len(), 0);
    budget.with_deferred_refund_notifications(|_| {
        insert(&mut set, &keys[0], &budget, &stats);
        let first_pointer = set.iter().next().unwrap().pointer();
        for key in &keys[1..] {
            insert(&mut set, key, &budget, &stats);
        }
        assert_eq!(stats.copies.load(SeqCst), 5);
        assert_eq!(
            stats.drops.load(SeqCst),
            0,
            "growth only moves initialized keys"
        );
        assert_eq!(
            set.iter().find(|key| key.number() == 30).unwrap().pointer(),
            first_pointer
        );
        without_allocations(|| {
            assert_eq!(set.iter().len(), 5);
            assert!(set.iter().map(Payload::number).eq([10, 20, 30, 40, 50]));
            assert!(
                set.iter()
                    .rev()
                    .map(Payload::number)
                    .eq([50, 40, 30, 20, 10])
            );
        });
        let old = (identity(&set), budget.reserved_bytes());
        let plan = without_allocations(|| {
            set.plan::<usize, Policy>(&keys[0], AllocationDemand::new())
                .unwrap()
        });
        assert_eq!(plan.demand().bytes(), 0);
        assert_eq!(plan.demand().allocations(), 0);
        let prepared = without_allocations(|| prepare(plan, &budget, &stats));
        without_allocations(|| drop(prepared.install()));
        assert_eq!((identity(&set), budget.reserved_bytes()), old);
        assert_eq!(stats.copies.load(SeqCst), 5);
        without_allocations(|| drop(set));
        assert_eq!(budget.reserved_bytes(), 5 * Layout::new::<usize>().size());
        without_allocations(|| drop(keys));
    });
    assert_eq!(stats.drops.load(SeqCst), 10);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn touch_exact_joined_capacity_and_one_byte_below_preserve_original_state() {
    let budget = AllocationBudget::new(8192);
    let stats = Stats::new();
    let key = Payload::input(7, &budget, &stats);
    let mut set = SortedTouches::new();
    let plan = without_allocations(|| {
        set.plan::<usize, Policy>(&key, AllocationDemand::new())
            .unwrap()
    });
    let demand = plan.demand();
    assert_eq!(
        demand.bytes(),
        Layout::new::<MaybeUninit<Payload>>().size() + Layout::new::<usize>().size()
    );
    assert_eq!(demand.allocations(), 2);
    let prior = budget.reserved_bytes();
    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - prior - demand.bytes() + 1)
        .unwrap();
    assert!(without_allocations(|| budget.try_reserve_bytes(demand.bytes())).is_err());
    drop(plan);
    assert_eq!(identity(&set), (0, 0, 0));
    assert_eq!(stats.copies.load(SeqCst), 0);
    budget.with_deferred_refund_notifications(|_| {
        without_allocations(|| drop(blocker));
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - prior - demand.bytes())
            .unwrap();
        let plan = without_allocations(|| {
            set.plan::<usize, Policy>(&key, AllocationDemand::new())
                .unwrap()
        });
        let prepared = prepare(plan, &budget, &stats);
        assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
        without_allocations(|| drop(prepared.install()));
        assert_eq!(set.iter().next().unwrap().number(), 7);
        // Fully reserved pool: destruction needs no new allocation or credit.
        without_allocations(|| drop(set));
        assert_eq!(budget.reserved_bytes(), prior + blocker.remaining_bytes());
        without_allocations(|| drop(blocker));
        without_allocations(|| drop(key));
    });
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn touch_preparation_abandonment_and_copy_panic_leave_old_array_and_keys_exact() {
    let budget = AllocationBudget::new(8192);
    let stats = Stats::new();
    let keys = [7, 8].map(|value| Payload::input(value, &budget, &stats));
    let mut set = SortedTouches::new();
    budget.with_deferred_refund_notifications(|_| {
        insert(&mut set, &keys[0], &budget, &stats);
        let before = (
            identity(&set),
            set.iter().next().unwrap().pointer(),
            budget.reserved_bytes(),
        );
        let plan = set
            .plan::<usize, Policy>(&keys[1], AllocationDemand::new())
            .unwrap();
        let prepared = prepare(plan, &budget, &stats);
        without_allocations(|| drop(prepared));
        assert_eq!(
            (
                identity(&set),
                set.iter().next().unwrap().pointer(),
                budget.reserved_bytes()
            ),
            before
        );
        stats.panic_copy.store(true, SeqCst);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let plan = set
                    .plan::<usize, Policy>(&keys[1], AllocationDemand::new())
                    .unwrap();
                drop(prepare(plan, &budget, &stats));
            }))
            .is_err()
        );
        assert_eq!(
            (
                identity(&set),
                set.iter().next().unwrap().pointer(),
                budget.reserved_bytes()
            ),
            before
        );
        insert(&mut set, &keys[1], &budget, &stats);
        assert!(set.iter().map(Payload::number).eq([7, 8]));
        without_allocations(|| drop(set));
        without_allocations(|| drop(keys));
    });
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn touch_arbitrary_key_drop_panic_drains_remaining_prefix_and_refunds_real_owners() {
    let budget = AllocationBudget::new(8192);
    let stats = Stats::new();
    let keys = [1, 2, 3].map(|value| Payload::input(value, &budget, &stats));
    let mut set = SortedTouches::new();
    budget.with_deferred_refund_notifications(|_| {
        for key in &keys {
            insert(&mut set, key, &budget, &stats);
        }
        let before = stats.drops.load(SeqCst);
        stats.panic_drop.store(2, SeqCst);
        assert!(catch_unwind(AssertUnwindSafe(|| drop(set))).is_err());
        assert_eq!(
            stats.drops.load(SeqCst),
            before + 3,
            "every stored key is destroyed once"
        );
        assert_eq!(
            budget.reserved_bytes(),
            keys.len() * Layout::new::<usize>().size()
        );
        without_allocations(|| drop(keys));
    });
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn touch_refused_payload_and_checked_growth_overflow_allocate_nothing() {
    without_allocations(|| {
        assert!(matches!(
            growth::<Payload>(usize::MAX, 0),
            Err(PlanningError::Overflow)
        ));
        assert!(matches!(
            growth::<[u8; 2]>(isize::MAX as usize, 0),
            Err(PlanningError::Overflow)
        ));
        let capacity = isize::MAX as usize / 4 + 1;
        let plan = growth::<[u8; 2]>(capacity, capacity).unwrap().unwrap();
        assert_eq!(
            plan.capacity,
            capacity + 1,
            "overflowed geometric layout falls back to checked exact growth"
        );
    });
    let budget = AllocationBudget::new(1024);
    let stats = Stats::new();
    let key = Payload::input(7, &budget, &stats);
    let mut set = SortedTouches::new();
    let before = budget.reserved_bytes();
    stats.unsupported.store(true, SeqCst);
    let result = without_allocations(|| set.plan::<usize, Policy>(&key, AllocationDemand::new()));
    assert!(matches!(result, Err(PlanningError::UnsupportedPayload)));
    drop(result);
    assert_eq!(identity(&set), (0, 0, 0));
    assert_eq!(budget.reserved_bytes(), before);
    assert_eq!(stats.copies.load(SeqCst), 0);
    without_allocations(|| drop((set, key)));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn touch_plan_extends_original_demand_and_preserves_exact_provider_remainder() {
    let budget = AllocationBudget::new(8192);
    let stats = Stats::new();
    let key = Payload::input(7, &budget, &stats);
    let mut set = SortedTouches::new();
    let mut base = AllocationDemand::new();
    let shell_layout = Layout::new::<u128>();
    base.add_layout(shell_layout).unwrap();
    budget.with_deferred_refund_notifications(|_| {
        let plan = without_allocations(|| set.plan::<usize, Policy>(&key, base).unwrap());
        let total = plan.demand();
        assert_eq!(
            total.bytes(),
            base.bytes()
                + Layout::new::<MaybeUninit<Payload>>().size()
                + Layout::new::<usize>().size()
        );
        assert_eq!(total.allocations(), base.allocations() + 2);
        let mut provider = Policy {
            reservation: budget.try_reserve_bytes(total.bytes()).unwrap(),
            stats: Arc::clone(&stats),
        };
        let prepared = plan.prepare::<usize, Policy>(&mut provider);
        assert!(provider.reservation.belongs_to(&budget));
        assert_eq!(provider.reservation.remaining_bytes(), base.bytes());
        // This independent real shell stands in for the untouched caller-owned
        // demand: one original reservation supplies both, with no re-admission.
        let shell_charge = provider.take_node_charge(shell_layout);
        let shell = Box::new(17_u128);
        assert_eq!(provider.reservation.remaining_bytes(), 0);
        without_allocations(|| drop(prepared.install()));
        without_allocations(|| drop(provider));
        let before = (
            identity(&set),
            budget.reserved_bytes(),
            stats.copies.load(SeqCst),
        );
        let duplicate = without_allocations(|| set.plan::<usize, Policy>(&key, base).unwrap());
        assert_eq!(
            duplicate.demand(),
            base,
            "duplicate preserves the complete original demand"
        );
        let mut provider = Policy {
            reservation: budget.try_reserve_bytes(base.bytes()).unwrap(),
            stats: Arc::clone(&stats),
        };
        let ready = without_allocations(|| duplicate.prepare::<usize, Policy>(&mut provider));
        assert_eq!(provider.reservation.remaining_bytes(), base.bytes());
        without_allocations(|| drop(ready.install()));
        without_allocations(|| drop(provider));
        assert_eq!(
            (
                identity(&set),
                budget.reserved_bytes(),
                stats.copies.load(SeqCst)
            ),
            before
        );
        without_allocations(|| drop(shell));
        without_allocations(|| drop(shell_charge));
        without_allocations(|| drop(set));
        without_allocations(|| drop(key));
    });
    assert_eq!(budget.reserved_bytes(), 0);
}
