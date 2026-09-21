//! Actual nonuniform nested allocations fund sibling and separator copies.

use super::*;
use crate::internals::bptree::node::L_CAPACITY;
use std::sync::{
    atomic::{AtomicUsize, Ordering::SeqCst},
    Arc,
};

struct NestedPool {
    used: AtomicUsize,
    limit: AtomicUsize,
    copies: AtomicUsize,
    fail_copy: AtomicUsize,
    panic_key: AtomicUsize,
}
impl NestedPool {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            used: AtomicUsize::new(0),
            limit: AtomicUsize::new(64 << 20),
            copies: AtomicUsize::new(0),
            fail_copy: AtomicUsize::new(0),
            panic_key: AtomicUsize::new(usize::MAX),
        })
    }
    fn reserve(self: &Arc<Self>, demand: AllocationDemand) -> Result<NestedPolicy, ()> {
        let used = self.used.load(SeqCst);
        let total = used.checked_add(demand.bytes()).ok_or(())?;
        if total > self.limit.load(SeqCst) {
            return Err(());
        }
        self.used.store(total, SeqCst);
        Ok(NestedPolicy {
            pool: Arc::clone(self),
            remaining: demand.bytes(),
        })
    }
}
struct NestedCharge {
    pool: Arc<NestedPool>,
    bytes: usize,
}
impl Drop for NestedCharge {
    fn drop(&mut self) {
        assert!(self.pool.used.fetch_sub(self.bytes, SeqCst) >= self.bytes);
    }
}
struct NestedPolicy {
    pool: Arc<NestedPool>,
    remaining: usize,
}
impl Drop for NestedPolicy {
    fn drop(&mut self) {
        assert!(self.pool.used.fetch_sub(self.remaining, SeqCst) >= self.remaining);
    }
}
impl NodeFunding for NestedPolicy {
    type Charge = NestedCharge;
    fn take_node_charge(&mut self, layout: Layout) -> NestedCharge {
        self.remaining = self
            .remaining
            .checked_sub(layout.size())
            .expect("one concrete reservation funds each actual nested/node layout");
        NestedCharge {
            pool: Arc::clone(&self.pool),
            bytes: layout.size(),
        }
    }
}
// Fields drop in declaration order: physical bytes before their original charge.
struct Bytes {
    order: usize,
    data: Box<[u8]>,
    _charge: NestedCharge,
    panic_drop: bool,
    unsupported: bool,
}
impl Debug for Bytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Bytes")
            .field(&self.order)
            .field(&self.data.len())
            .finish()
    }
}
impl Clone for Bytes {
    fn clone(&self) -> Self {
        panic!("ordinary unfunded payload clone")
    }
}
impl PartialEq for Bytes {
    fn eq(&self, other: &Self) -> bool {
        self.order == other.order
    }
}
impl Eq for Bytes {}
impl PartialOrd for Bytes {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Bytes {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.order.cmp(&other.order)
    }
}
impl std::borrow::Borrow<usize> for Bytes {
    fn borrow(&self) -> &usize {
        &self.order
    }
}
impl Drop for Bytes {
    fn drop(&mut self) {
        assert!(!self.panic_drop, "owned query destructor failed under pair");
        assert!(
            self._charge
                .pool
                .panic_key
                .compare_exchange(self.order, usize::MAX, SeqCst, SeqCst)
                .is_err(),
            "stored or boundary key destructor failed"
        );
    }
}
impl NestedPolicy {
    fn copy(&mut self, value: &Bytes) -> Bytes {
        let next = self.pool.copies.fetch_add(1, SeqCst) + 1;
        assert_ne!(
            self.pool.fail_copy.load(SeqCst),
            next,
            "selected funded payload copy failed"
        );
        let charge = self.take_node_charge(Layout::array::<u8>(value.data.len()).unwrap());
        Bytes {
            order: value.order,
            data: value.data.to_vec().into_boxed_slice(),
            _charge: charge,
            panic_drop: false,
            unsupported: value.unsupported,
        }
    }
}
fn payload_plan(value: &Bytes, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
    if value.unsupported {
        return Err(PlanningError::UnsupportedPayload);
    }
    demand.add_layout(Layout::array::<u8>(value.data.len()).map_err(|_| PlanningError::Overflow)?)
}
impl NodeCloning<Bytes, Bytes> for NestedPolicy {
    fn clone_key(&mut self, key: &Bytes) -> Bytes {
        self.copy(key)
    }
    fn clone_value(&mut self, value: &Bytes) -> Bytes {
        self.copy(value)
    }
}
impl NodeCloning<Bytes, Option<Bytes>> for NestedPolicy {
    fn clone_key(&mut self, key: &Bytes) -> Bytes {
        self.copy(key)
    }
    fn clone_value(&mut self, value: &Option<Bytes>) -> Option<Bytes> {
        value.as_ref().map(|v| self.copy(v))
    }
}
impl ClonePlanning<Bytes, Bytes> for NestedPolicy {
    fn plan_key(key: &Bytes, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        payload_plan(key, demand)
    }
    fn plan_value(value: &Bytes, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        payload_plan(value, demand)
    }
}
impl ClonePlanning<Bytes, Option<Bytes>> for NestedPolicy {
    fn plan_key(key: &Bytes, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        payload_plan(key, demand)
    }
    fn plan_value(
        value: &Option<Bytes>,
        demand: &mut AllocationDemand,
    ) -> Result<(), PlanningError> {
        if let Some(value) = value {
            payload_plan(value, demand)?;
        }
        Ok(())
    }
}
type NestedMap<V> = BptreeMap<Bytes, V, Prepaid<NestedPolicy>>;
fn bytes(pool: &Arc<NestedPool>, order: usize, length: usize) -> Bytes {
    let mut demand = AllocationDemand::new();
    let layout = Layout::array::<u8>(length).unwrap();
    demand.add_layout(layout).unwrap();
    let mut provider = pool.reserve(demand).unwrap();
    let charge = provider.take_node_charge(layout);
    Bytes {
        order,
        data: vec![(order % 251) as u8; length].into_boxed_slice(),
        _charge: charge,
        panic_drop: false,
        unsupported: false,
    }
}
fn key(pool: &Arc<NestedPool>, order: usize) -> Bytes {
    bytes(pool, order, 1 + (order * 149) % 4096)
}
fn nested_maps(pool: &Arc<NestedPool>) -> (NestedMap<Bytes>, NestedMap<Option<Bytes>>) {
    let current = NestedMap::try_new_with_node_custody(|d| pool.reserve(d)).unwrap();
    let undo = NestedMap::try_new_with_node_custody(|d| pool.reserve(d)).unwrap();
    let mut seed = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
    for index in 0..96 {
        seed.try_insert_admitted(
            key(pool, index),
            bytes(pool, index + 1000, 1 + (index * 281) % 3072),
            |d| pool.reserve(d),
        )
        .unwrap();
    }
    seed.commit();
    (current, undo)
}

#[test]
fn nonuniform_sibling_and_repair_payloads_fit_complete_demand_and_old_readers() {
    for reversed in [false, true] {
        let pool = NestedPool::new();
        let (current, undo) = nested_maps(&pool);
        let old = current.read();
        let old_root = old.inner.as_ref().get_root();
        let old_pointer = old.get(&0).unwrap().data.as_ptr();
        let mut cw = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
        let mut uw = undo.try_write_admitted(|d| pool.reserve(d)).unwrap();
        for step in 0..96 {
            let index = if reversed { 95 - step } else { step };
            let query = key(&pool, index);
            let original_input = query.data.as_ptr();
            let used = pool.used.load(SeqCst);
            let copies = pool.copies.load(SeqCst);
            let demand = Cell::new(AllocationDemand::new());
            let (query, error) = without_allocations(|| {
                cw.try_remove_with_undo_admitted(&mut uw, query, |d, q| {
                    assert_eq!(q.data.as_ptr(), original_input);
                    assert_eq!(pool.copies.load(SeqCst), copies);
                    demand.set(d);
                    Err::<NestedPolicy, _>(())
                })
                .err()
                .unwrap()
            });
            assert!(matches!(error, PairRemoveError::Refused(())));
            assert_eq!(query.data.as_ptr(), original_input);
            assert_eq!(pool.used.load(SeqCst), used);
            pool.limit.store(used + demand.get().bytes() - 1, SeqCst);
            let (query, error) = without_allocations(|| {
                cw.try_remove_with_undo_admitted(&mut uw, query, |d, _| {
                    assert_eq!(d, demand.get());
                    pool.reserve(d)
                })
                .err()
                .unwrap()
            });
            assert!(matches!(error, PairRemoveError::Refused(())));
            assert_eq!(pool.copies.load(SeqCst), copies);
            pool.limit.store(used + demand.get().bytes(), SeqCst);
            let removed = cw
                .try_remove_with_undo_admitted(&mut uw, query, |d, _| {
                    assert_eq!(d, demand.get());
                    pool.reserve(d)
                })
                .unwrap()
                .unwrap();
            assert_eq!(removed.order, index + 1000);
            assert_eq!(
                removed.data.as_ref(),
                old.get(&index).unwrap().data.as_ref()
            );
            assert_eq!(
                uw.get(&index).unwrap().as_ref().unwrap().data.as_ref(),
                old.get(&index).unwrap().data.as_ref()
            );
            assert_eq!(cw.len(), 95 - step);
            assert!(cw.inner.as_ref().verify() && uw.inner.as_ref().verify());
            assert_eq!(old.inner.as_ref().get_root(), old_root);
            assert_eq!(old.get(&0).unwrap().data.as_ptr(), old_pointer);
            drop(removed);
            pool.limit.store(64 << 20, SeqCst);
        }
        assert!(cw.is_empty());
        without_allocations(|| {
            cw.commit();
            uw.commit();
        });
        assert!(current.read().is_empty());
        assert_eq!(old.get(&0).unwrap().data.as_ptr(), old_pointer);
        without_allocations(|| drop(old));
        without_allocations(|| drop((current, undo)));
        assert_eq!(pool.used.load(SeqCst), 0);
    }
}

#[test]
fn nonuniform_copy_and_query_drop_failures_poison_both_original_parents() {
    // First run measures actual copies at a leaf-emptying deletion. Every later
    // run fails one of those exact copies, including path/sibling/repair sources.
    let mut actual_copies = 0;
    for fault in 0.. {
        if fault > actual_copies + 3 && fault != 0 {
            break;
        }
        let pool = NestedPool::new();
        let (current, undo) = nested_maps(&pool);
        let mut cw = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
        let mut uw = undo.try_write_admitted(|d| pool.reserve(d)).unwrap();
        for index in 0..L_CAPACITY - 1 {
            drop(
                cw.try_remove_with_undo_admitted(&mut uw, key(&pool, index), |d, _| {
                    pool.reserve(d)
                })
                .unwrap(),
            );
        }
        let mut cp = cw.checkpoint().unwrap();
        let mut up = uw.checkpoint().unwrap();
        let mut query = key(&pool, L_CAPACITY - 1);
        if fault == actual_copies + 1 && fault != 0 {
            query.panic_drop = true;
        }
        let before = pool.copies.load(SeqCst);
        let result = catch_unwind(AssertUnwindSafe(|| {
            cp.try_remove_with_undo_admitted(&mut up, query, |d, _| {
                let provider = pool.reserve(d)?;
                if fault == actual_copies + 2 {
                    pool.panic_key.store(L_CAPACITY - 1, SeqCst);
                }
                if fault == actual_copies + 3 {
                    pool.panic_key.store(L_CAPACITY, SeqCst);
                }
                if fault != 0 && fault <= actual_copies {
                    pool.fail_copy.store(before + fault, SeqCst);
                }
                Ok::<_, ()>(provider)
            })
        }));
        let pending_key_fault = pool.panic_key.swap(usize::MAX, SeqCst);
        if fault == 0 {
            drop(result.unwrap().unwrap());
            actual_copies = pool.copies.load(SeqCst) - before;
            assert!(actual_copies > 4, "real path/sibling and undo copies");
        } else {
            assert!(
                result.is_err(),
                "selected actual copy, query, stored or boundary key destructor must run"
            );
            if fault >= actual_copies + 2 {
                assert_eq!(pending_key_fault, usize::MAX);
            }
            assert!(catch_unwind(AssertUnwindSafe(|| cp.len())).is_err());
            assert!(catch_unwind(AssertUnwindSafe(|| up.len())).is_err());
        }
        drop((cp, up));
        if fault != 0 {
            assert!(catch_unwind(AssertUnwindSafe(|| cw.len())).is_err());
            assert!(catch_unwind(AssertUnwindSafe(|| uw.len())).is_err());
        }
        without_allocations(|| drop((cw, uw)));
        assert_eq!(current.read().len(), 96);
        assert!(undo.read().is_empty());
        without_allocations(|| drop((current, undo)));
        assert_eq!(pool.used.load(SeqCst), 0);
    }
}

#[test]
fn replacement_input_key_drop_preserves_previous_value_custody() {
    let pool = NestedPool::new();
    let (current, undo) = nested_maps(&pool);
    let old = current.read();
    let original = old.get(&7).unwrap().data.as_ptr();
    let mut writer = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
    let mut checkpoint = writer.checkpoint().unwrap();
    let mut query = key(&pool, 7);
    query.panic_drop = true;
    let value = bytes(&pool, 2007, 1111);
    let copies = pool.copies.load(SeqCst);
    let result = catch_unwind(AssertUnwindSafe(|| {
        checkpoint.try_insert_admitted(query, value, |d| pool.reserve(d))
    }));
    assert!(result.is_err(), "consumed replacement key must unwind");
    assert!(pool.copies.load(SeqCst) > copies, "actual paid path clone");
    assert!(catch_unwind(AssertUnwindSafe(|| checkpoint.len())).is_err());
    drop(checkpoint);
    assert!(catch_unwind(AssertUnwindSafe(|| writer.len())).is_err());
    without_allocations(|| drop(writer));
    assert_eq!(current.read().get(&7).unwrap().data.as_ptr(), original);
    assert_eq!(old.get(&7).unwrap().data.as_ptr(), original);
    assert!(undo.read().is_empty());
    without_allocations(|| drop(old));
    without_allocations(|| drop((current, undo)));
    assert_eq!(
        pool.used.load(SeqCst),
        0,
        "previous and replacement values must both release their exact original charges"
    );
}
