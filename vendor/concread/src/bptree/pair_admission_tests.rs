//! Finite charges and paired checkpoint custody for closed insertion.

use super::*;
use crate::internals::bptree::node::allocation_tests::without_allocations;
use crate::internals::bptree::node::{TXID_MASK, TXID_SHF};
use crate::internals::bptree::states::LeafInsertState;
use std::{
    cell::Cell,
    panic::{catch_unwind, AssertUnwindSafe},
    rc::Rc,
};

#[derive(Default)]
struct Pool {
    limit: Cell<usize>,
    used: Cell<usize>,
    callbacks: Cell<usize>,
    takes: Cell<usize>,
    clones: Cell<usize>,
    drops: Cell<usize>,
    panic_drop: Cell<bool>,
    panic_clone: Cell<u8>,
    arm_charge_panic_on_provider_drop: Cell<bool>,
    panic_next_charge: Cell<bool>,
}
impl Pool {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            limit: Cell::new(4 * 1024 * 1024),
            ..Self::default()
        })
    }
    fn reserve(self: &Rc<Self>, demand: AllocationDemand) -> Result<Policy, ()> {
        self.callbacks.set(self.callbacks.get() + 1);
        let next = self.used.get().checked_add(demand.bytes()).ok_or(())?;
        if next > self.limit.get() {
            return Err(());
        }
        self.used.set(next);
        Ok(Policy {
            pool: Rc::clone(self),
            remaining: demand.bytes(),
        })
    }
    fn copied(&self, role: u8) {
        self.clones.set(self.clones.get() + 1);
        if self.panic_clone.get() == role {
            self.panic_clone.set(0);
            panic!("selected payload clone failed");
        }
    }
}
struct Charge {
    pool: Rc<Pool>,
    bytes: usize,
}
impl Drop for Charge {
    fn drop(&mut self) {
        self.pool
            .used
            .set(self.pool.used.get().checked_sub(self.bytes).unwrap());
        assert!(
            !self.pool.panic_next_charge.replace(false),
            "original charged allocation cleanup failed"
        );
    }
}
struct Policy {
    pool: Rc<Pool>,
    remaining: usize,
}
impl NodeFunding for Policy {
    type Charge = Charge;
    fn take_node_charge(&mut self, layout: Layout) -> Charge {
        self.remaining = self
            .remaining
            .checked_sub(layout.size())
            .expect("single reservation covers each actual layout");
        self.pool.takes.set(self.pool.takes.get() + 1);
        Charge {
            pool: Rc::clone(&self.pool),
            bytes: layout.size(),
        }
    }
}
impl Drop for Policy {
    fn drop(&mut self) {
        self.pool
            .used
            .set(self.pool.used.get().checked_sub(self.remaining).unwrap());
        self.pool.drops.set(self.pool.drops.get() + 1);
        if self.pool.arm_charge_panic_on_provider_drop.replace(false) {
            self.pool.panic_next_charge.set(true);
        }
        assert!(
            !self.pool.panic_drop.replace(false),
            "unused provider cleanup failed"
        );
    }
}
impl NodeCloning<usize, usize> for Policy {
    fn clone_key(&mut self, key: &usize) -> usize {
        self.pool.copied(1);
        *key
    }
    fn clone_value(&mut self, value: &usize) -> usize {
        self.pool.copied(1);
        *value
    }
}
impl NodeCloning<usize, Option<usize>> for Policy {
    fn clone_key(&mut self, key: &usize) -> usize {
        self.pool.copied(3);
        *key
    }
    fn clone_value(&mut self, value: &Option<usize>) -> Option<usize> {
        self.pool.copied(2);
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
impl ClonePlanning<usize, Option<usize>> for Policy {
    fn plan_key(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &Option<usize>, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}
// Scalar payloads own no nested allocations. MV's separate witness covers
// actual nested key/value charges; these controls exercise engine ownership.
type Map<V> = BptreeMap<usize, V, Prepaid<Policy>>;
type Owner<V> = BptreeMapOwned<usize, V, Prepaid<Policy>>;
fn map<V: Clone + Send + Sync + 'static>(pool: &Rc<Pool>) -> Map<V>
where
    Policy: ClonePlanning<usize, V>,
{
    Map::try_new_with_node_custody(|d| pool.reserve(d)).unwrap()
}
fn start<V: Clone + Send + Sync + 'static>(map: &Map<V>, pool: &Rc<Pool>) -> Owner<V>
where
    Policy: ClonePlanning<usize, V>,
{
    map.try_write_admitted(|d| pool.reserve(d))
        .unwrap()
        .detach()
}
fn put<V: Clone + Send + Sync + 'static>(
    map: &Map<V>,
    owner: Owner<V>,
    key: usize,
    value: V,
    pool: &Rc<Pool>,
) -> Owner<V>
where
    Policy: ClonePlanning<usize, V>,
{
    map.try_insert_owned_admitted(owner, key, value, |d| pool.reserve(d))
        .unwrap_or_else(|_| panic!("finite fixture insertion"))
        .0
}
#[derive(Debug, PartialEq, Eq)]
struct Identity {
    cursor: usize,
    root: usize,
    txid: u64,
    length: usize,
    tracking: [(usize, usize); 2],
    backing: [usize; 2],
}
fn identity<V: Clone>(cursor: &CursorWrite<usize, V, Prepaid<Policy>>) -> Identity
where
    Policy: NodeCloning<usize, V>,
{
    Identity {
        cursor: cursor as *const _ as usize,
        root: cursor.get_root() as usize,
        txid: cursor.get_txid(),
        length: cursor.len(),
        tracking: cursor.admitted_tracking(),
        backing: cursor.admitted_tracking_addresses(),
    }
}
fn pair(
    current: &Map<usize>,
    c: Owner<usize>,
    undo: &Map<Option<usize>>,
    u: Owner<Option<usize>>,
    key: usize,
    value: usize,
    pool: &Rc<Pool>,
) -> ((Owner<usize>, Owner<Option<usize>>), Option<usize>) {
    current
        .try_insert_with_undo_owned_admitted(c, undo, u, key, value, |d| pool.reserve(d))
        .unwrap_or_else(|_| panic!("finite joined insertion"))
}

#[test]
fn exact_joined_limit_and_one_byte_below_preserve_original_owners() {
    let pool = Pool::new();
    let current = map::<usize>(&pool);
    let undo = map::<Option<usize>>(&pool);
    let c = start(&current, &pool);
    let u = start(&undo, &pool);
    let original = (identity(c.inner.as_ref()), identity(u.inner.as_ref()));
    let used = pool.used.get();
    let takes = pool.takes.get();
    let clones = pool.clones.get();
    let plan = without_allocations(|| {
        plan_pair::<_, _, Policy>(c.inner.as_ref(), u.inner.as_ref(), &7).unwrap()
    });
    let demand = plan.demand;
    assert!(demand.bytes() > 0);
    pool.limit.set(used + demand.bytes() - 1);
    let callbacks = pool.callbacks.get();
    let ((c, u, key, value), error) = without_allocations(|| {
        current
            .try_insert_with_undo_owned_admitted(c, &undo, u, 7, 70, |d| {
                assert_eq!(d, demand);
                pool.reserve(d)
            })
            .err()
            .expect("one byte below")
    });
    assert!(matches!(error, PairInsertError::Refused(())));
    assert_eq!(
        (identity(c.inner.as_ref()), identity(u.inner.as_ref())),
        original
    );
    assert_eq!((key, value), (7, 70));
    assert_eq!(
        (pool.used.get(), pool.takes.get(), pool.clones.get()),
        (used, takes, clones)
    );
    assert_eq!(pool.callbacks.get(), callbacks + 1);
    pool.limit.set(used + demand.bytes());
    let drops = pool.drops.get();
    let ((c, u), previous) = pair(&current, c, &undo, u, key, value, &pool);
    assert_eq!(previous, None);
    assert_eq!(pool.callbacks.get(), callbacks + 2);
    assert_eq!(pool.drops.get(), drops + 1);
    assert!(pool.used.get() <= pool.limit.get());
    assert_eq!(c.inner.as_ref() as *const _ as usize, original.0.cursor);
    assert_eq!(u.inner.as_ref() as *const _ as usize, original.1.cursor);
    assert_eq!(c.get(&7), Some(&70));
    assert_eq!(u.get(&7), Some(&None));
    assert!(current.read().is_empty() && undo.read().is_empty());
    without_allocations(|| drop((c, u, current, undo)));
    assert_eq!(pool.used.get(), 0);
}

#[test]
fn joined_growth_keeps_first_none_and_some_without_rewriting_existing_undo() {
    let pool = Pool::new();
    let current = map::<usize>(&pool);
    let undo = map::<Option<usize>>(&pool);
    let seed = put(&current, start(&current, &pool), 7, 70, &pool);
    current
        .try_write_owned(seed)
        .unwrap_or_else(|_| panic!("seed writer"))
        .commit();
    let mut c = start(&current, &pool);
    let mut u = start(&undo, &pool);
    for key in 0..96 {
        let callbacks = pool.callbacks.get();
        let ((next_c, next_u), previous) = pair(&current, c, &undo, u, key, key * 10 + 1, &pool);
        assert_eq!(previous, (key == 7).then_some(70));
        assert_eq!(pool.callbacks.get(), callbacks + 1);
        assert_eq!(next_u.get(&key), Some(&((key == 7).then_some(70))));
        assert!(next_c.inner.as_ref().verify() && next_u.inner.as_ref().verify());
        c = next_c;
        u = next_u;
    }
    let unchanged = identity(u.inner.as_ref());
    for key in 0..96 {
        let ((next_c, next_u), previous) = pair(&current, c, &undo, u, key, key * 10 + 2, &pool);
        assert_eq!(previous, Some(key * 10 + 1));
        assert_eq!(identity(next_u.inner.as_ref()), unchanged);
        assert_eq!(next_u.get(&key), Some(&((key == 7).then_some(70))));
        c = next_c;
        u = next_u;
    }
    assert_eq!(current.read().get(&7), Some(&70));
    assert_eq!(current.read().len(), 1);
    assert!(undo.read().is_empty());
    drop((c, u, current, undo));
    assert_eq!(pool.used.get(), 0);
}

#[test]
fn both_checkpoints_abort_to_prior_private_roots_without_credit() {
    let pool = Pool::new();
    let current = map::<usize>(&pool);
    let undo = map::<Option<usize>>(&pool);
    let c = put(&current, start(&current, &pool), 3, 30, &pool);
    let u = put(&undo, start(&undo, &pool), 3, Some(29), &pool);
    let mut cw = current
        .inner
        .try_write_owned(c.inner)
        .unwrap_or_else(|_| panic!("current"));
    let mut uw = undo
        .inner
        .try_write_owned(u.inner)
        .unwrap_or_else(|_| panic!("undo"));
    let before = (
        identity(cw.as_ref()),
        identity(uw.as_ref()),
        pool.used.get(),
    );
    let plan =
        without_allocations(|| plan_pair::<_, _, Policy>(cw.as_ref(), uw.as_ref(), &7).unwrap());
    let provider = pool.reserve(plan.demand).unwrap();
    let callbacks = pool.callbacks.get();
    let mut cc = cw.as_mut().checkpoint().unwrap();
    let mut uc = uw.as_mut().checkpoint().unwrap();
    assert_eq!(
        execute_pair(&mut cc, Some(&mut uc), plan, 7, 70, provider),
        None
    );
    pool.limit.set(pool.used.get());
    without_allocations(|| drop((cc, uc)));
    assert_eq!(
        (
            identity(cw.as_ref()),
            identity(uw.as_ref()),
            pool.used.get()
        ),
        before
    );
    assert_eq!(pool.callbacks.get(), callbacks);
    assert_eq!(cw.as_ref().search(&3), Some(&30));
    assert_eq!(uw.as_ref().search(&3), Some(&Some(29)));
    assert!(cw.as_ref().search(&7).is_none() && uw.as_ref().search(&7).is_none());
    without_allocations(|| drop((cw, uw)));
    without_allocations(|| drop((current, undo)));
    assert_eq!(pool.used.get(), 0);
}

// Build one real leaf with its correct generation and exact finite charges,
// avoiding impractically many commits without forging a cursor/root tag pair.
fn at_generation<V: Clone + Send + Sync + 'static>(
    pool: &Rc<Pool>,
    txid: u64,
    entry: Option<(usize, V)>,
) -> Map<V>
where
    Policy: ClonePlanning<usize, V>,
{
    let layouts = MapCell::<usize, V, Prepaid<Policy>>::initial_allocation_layouts();
    let mut demand = AllocationDemand::new();
    for layout in [
        Layout::new::<CachePadded<Leaf<usize, V, Charge>>>(),
        layouts.root,
        layouts.reader,
    ] {
        demand.add_layout(layout).unwrap();
    }
    let mut provider = pool.reserve(demand).unwrap();
    let leaf = Node::<usize, V, Charge>::new_leaf(txid, &mut provider);
    let size = usize::from(entry.is_some());
    if let Some((key, value)) = entry {
        // SAFETY: this newly allocated empty leaf has one exclusive owner.
        assert!(matches!(
            unsafe { &mut *leaf }.insert_or_update(key, value, &mut provider),
            LeafInsertState::Ok(None)
        ));
    }
    let root = leaf.cast::<Node<usize, V, Charge>>();
    Node::make_ro_raw(root);
    let source = SuperBlock::<usize, V, Prepaid<Policy>> { root, size, txid };
    let charges = InitialCharges {
        root: provider.take_node_charge(layouts.root),
        reader: provider.take_node_charge(layouts.reader),
    };
    Map {
        inner: LinCowCell::new_charged(source, charges),
    }
}

#[test]
fn undo_generation_is_required_only_for_missing_first_preimage() {
    let maximum = (TXID_MASK >> TXID_SHF) - 1;
    for entry in [None, Some((7, None)), Some((7, Some(69)))] {
        let pool = Pool::new();
        let current = map::<usize>(&pool);
        let undo = at_generation(&pool, maximum - 1, entry);
        let c = start(&current, &pool);
        let u = start(&undo, &pool);
        let before = identity(u.inner.as_ref());
        assert_eq!(before.txid, maximum);
        let callbacks = pool.callbacks.get();
        if entry.is_none() {
            let ((c, u, key, value), error) = without_allocations(|| {
                current
                    .try_insert_with_undo_owned_admitted(
                        c,
                        &undo,
                        u,
                        7,
                        70,
                        |_| -> Result<Policy, ()> { panic!("exhausted undo before callback") },
                    )
                    .err()
                    .unwrap()
            });
            assert!(matches!(
                error,
                PairInsertError::Planning(PlanningError::Overflow)
            ));
            assert_eq!((key, value), (7, 70));
            assert_eq!(identity(u.inner.as_ref()), before);
            assert_eq!(pool.callbacks.get(), callbacks);
            drop((c, u));
        } else {
            let ((c, u), previous) = pair(&current, c, &undo, u, 7, 70, &pool);
            assert_eq!(previous, None);
            assert_eq!(identity(u.inner.as_ref()), before);
            assert_eq!(u.get(&7), entry.as_ref().map(|(_, value)| value));
            assert_eq!(pool.callbacks.get(), callbacks + 1);
            drop((c, u));
        }
        drop((current, undo));
        assert_eq!(pool.used.get(), 0);
    }
}

#[test]
fn current_generation_refusal_preserves_both_inputs_before_callback() {
    let pool = Pool::new();
    let current = at_generation::<usize>(&pool, (TXID_MASK >> TXID_SHF) - 2, None);
    let undo = map::<Option<usize>>(&pool);
    let c = start(&current, &pool);
    let u = start(&undo, &pool);
    let before = (identity(c.inner.as_ref()), identity(u.inner.as_ref()));
    let ((c, u, key, value), error) = without_allocations(|| {
        current
            .try_insert_with_undo_owned_admitted(c, &undo, u, 7, 70, |_| -> Result<Policy, ()> {
                panic!("exhausted current before callback")
            })
            .err()
            .unwrap()
    });
    assert!(matches!(
        error,
        PairInsertError::Planning(PlanningError::Overflow)
    ));
    assert_eq!(
        (identity(c.inner.as_ref()), identity(u.inner.as_ref())),
        before
    );
    assert_eq!((key, value), (7, 70));
    drop((c, u, current, undo));
    assert_eq!(pool.used.get(), 0);
}

#[test]
fn foreign_stale_busy_and_poisoned_roles_refuse_before_joined_admission() {
    for role in 0..2 {
        for fault in 0..4 {
            let pool = Pool::new();
            let current = map::<usize>(&pool);
            let undo = map::<Option<usize>>(&pool);
            let other_current = map::<usize>(&pool);
            let other_undo = map::<Option<usize>>(&pool);
            let c = start(
                if role == 0 && fault == 0 {
                    &other_current
                } else {
                    &current
                },
                &pool,
            );
            let u = start(
                if role == 1 && fault == 0 {
                    &other_undo
                } else {
                    &undo
                },
                &pool,
            );
            let before = (identity(c.inner.as_ref()), identity(u.inner.as_ref()));
            let mut current_lock = None;
            let mut undo_lock = None;
            match (role, fault) {
                (0, 1) => current
                    .try_write_admitted(|d| pool.reserve(d))
                    .unwrap()
                    .commit(),
                (1, 1) => undo
                    .try_write_admitted(|d| pool.reserve(d))
                    .unwrap()
                    .commit(),
                (0, 2) => {
                    current_lock = Some(current.try_write_admitted(|d| pool.reserve(d)).unwrap())
                }
                (1, 2) => undo_lock = Some(undo.try_write_admitted(|d| pool.reserve(d)).unwrap()),
                (_, 3) => {
                    assert!(catch_unwind(AssertUnwindSafe(|| {
                        if role == 0 {
                            let _held = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
                            panic!("poison current original lock");
                        } else {
                            let _held = undo.try_write_admitted(|d| pool.reserve(d)).unwrap();
                            panic!("poison undo original lock");
                        }
                    }))
                    .is_err());
                }
                _ => {}
            }
            let ((c, u, key, value), error) = without_allocations(|| {
                current
                    .try_insert_with_undo_owned_admitted(
                        c,
                        &undo,
                        u,
                        7,
                        70,
                        |_| -> Result<Policy, ()> { panic!("owner refusal before callback") },
                    )
                    .err()
                    .unwrap()
            });
            let inner = match (role, error) {
                (0, PairInsertError::Current(e)) | (1, PairInsertError::Undo(e)) => e,
                _ => panic!("exact failing role"),
            };
            assert!(matches!(
                (fault, inner),
                (0 | 1, OwnedWriteError::Changed)
                    | (2, OwnedWriteError::Busy)
                    | (3, OwnedWriteError::Poisoned)
            ));
            assert_eq!(
                (identity(c.inner.as_ref()), identity(u.inner.as_ref())),
                before
            );
            assert_eq!((key, value), (7, 70));
            drop((current_lock, undo_lock));
            drop((c, u, current, undo, other_current, other_undo));
            assert_eq!(pool.used.get(), 0);
        }
    }
}

#[test]
fn callback_clone_and_remainder_drop_panics_poison_both_without_publication() {
    for fault in 0..4 {
        let pool = Pool::new();
        let current = map::<usize>(&pool);
        let undo = map::<Option<usize>>(&pool);
        let c = put(&current, start(&current, &pool), 3, 30, &pool);
        let u = put(&undo, start(&undo, &pool), 3, Some(29), &pool);
        let roots = (
            current.read().inner.as_ref().get_root(),
            undo.read().inner.as_ref().get_root(),
        );
        let takes = pool.takes.get();
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _ = current.try_insert_with_undo_owned_admitted(
                c,
                &undo,
                u,
                7,
                70,
                |d| -> Result<Policy, ()> {
                    if fault == 0 {
                        panic!("joined callback failed");
                    }
                    let provider = pool.reserve(d)?;
                    if fault == 1 {
                        pool.panic_clone.set(1);
                    }
                    if fault == 2 {
                        pool.panic_clone.set(2);
                    }
                    if fault == 3 {
                        pool.panic_drop.set(true);
                    }
                    Ok(provider)
                },
            );
        }))
        .is_err());
        assert!(current.is_poisoned() && undo.is_poisoned());
        assert_eq!(
            (
                current.read().inner.as_ref().get_root(),
                undo.read().inner.as_ref().get_root()
            ),
            roots
        );
        assert!(current.read().is_empty() && undo.read().is_empty());
        if fault >= 2 {
            assert!(
                pool.takes.get() > takes,
                "panic follows real admitted allocations"
            );
        }
        let current_refused = without_allocations(|| {
            current.try_write_admitted::<()>(|_| panic!("current poisoned before callback"))
        });
        let undo_refused = without_allocations(|| {
            undo.try_write_admitted::<()>(|_| panic!("undo poisoned before callback"))
        });
        assert!(matches!(
            current_refused,
            Err(InsertAdmissionError::Poisoned)
        ));
        assert!(matches!(undo_refused, Err(InsertAdmissionError::Poisoned)));
        drop((current_refused, undo_refused));
        drop((current, undo));
        assert_eq!(pool.used.get(), 0);
    }
}

#[test]
fn single_borrowed_edit_keeps_failure_armed_through_provider_drop() {
    let pool = Pool::new();
    let current = map::<usize>(&pool);
    let mut writer = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = writer.try_insert_admitted(7, 70, |d| {
            let provider = pool.reserve(d)?;
            pool.panic_drop.set(true);
            Ok::<_, ()>(provider)
        });
    }))
    .is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| writer.len())).is_err());
    assert!(current.read().is_empty());
    drop(writer);
    drop(current);
    assert_eq!(pool.used.get(), 0);
}

#[test]
fn joined_sum_rejects_byte_and_allocation_overflow_without_changing_demand() {
    for (base, extra) in [
        (
            AllocationDemand {
                bytes: usize::MAX,
                allocations: 0,
            },
            AllocationDemand {
                bytes: 1,
                allocations: 0,
            },
        ),
        (
            AllocationDemand {
                bytes: 0,
                allocations: usize::MAX,
            },
            AllocationDemand {
                bytes: 0,
                allocations: 1,
            },
        ),
    ] {
        let mut demand = base;
        assert_eq!(
            without_allocations(|| demand.add(extra, 1)),
            Err(PlanningError::Overflow)
        );
        assert_eq!(demand, base);
    }
}

#[path = "borrowed_pair_tests.rs"]
mod borrowed;
