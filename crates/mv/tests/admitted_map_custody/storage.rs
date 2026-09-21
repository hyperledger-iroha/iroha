//! The production MV Storage family using its original finite allocation pool.
//! Node charges are native AllocationCharge; nested payload frees retain the
//! parent harness's physical allocator witness. Control/World funding is not claimed.
use super::*;
use mv::storage::{AdmittedBlockError, AdmittedStorageError, AdmittedStoragePolicy, Storage};
use std::cell::RefCell;

thread_local! {
    static STORAGE_COUNTERS: RefCell<Option<Arc<Counters>>> = const { RefCell::new(None) };
    static FACTORY_FAULT: Cell<u8> = const { Cell::new(0) };
    static FOREIGN_POOL: RefCell<Option<AllocationBudget>> = const { RefCell::new(None) };
}

struct PolicyContext;
impl PolicyContext {
    fn new(counters: &Arc<Counters>) -> Self {
        STORAGE_COUNTERS.with(|slot| assert!(slot.replace(Some(Arc::clone(counters))).is_none()));
        Self
    }
}
impl Drop for PolicyContext {
    fn drop(&mut self) {
        STORAGE_COUNTERS.with(|slot| assert!(slot.take().is_some()));
        FACTORY_FAULT.with(|mode| mode.set(0));
        FOREIGN_POOL.with(|slot| drop(slot.take()));
    }
}

struct NativeStoragePolicy(Policy);
impl NodeFunding for NativeStoragePolicy {
    type Charge = AllocationCharge;
    fn take_node_charge(&mut self, layout: Layout) -> AllocationCharge {
        self.0
            .reservation
            .try_split(layout)
            .expect("complete original node demand")
    }
}
impl NodeCloning<Payload, Payload> for NativeStoragePolicy {
    fn clone_key(&mut self, key: &Payload) -> Payload {
        <Policy as NodeCloning<Payload, Payload>>::clone_key(&mut self.0, key)
    }
    fn clone_value(&mut self, value: &Payload) -> Payload {
        <Policy as NodeCloning<Payload, Payload>>::clone_value(&mut self.0, value)
    }
}
impl NodeCloning<Payload, Option<Payload>> for NativeStoragePolicy {
    fn clone_key(&mut self, key: &Payload) -> Payload {
        <Policy as NodeCloning<Payload, Option<Payload>>>::clone_key(&mut self.0, key)
    }
    fn clone_value(&mut self, value: &Option<Payload>) -> Option<Payload> {
        <Policy as NodeCloning<Payload, Option<Payload>>>::clone_value(&mut self.0, value)
    }
}
impl ClonePlanning<Payload, Payload> for NativeStoragePolicy {
    fn plan_key(key: &Payload, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        <Policy as ClonePlanning<Payload, Payload>>::plan_key(key, demand)
    }
    fn plan_value(value: &Payload, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        <Policy as ClonePlanning<Payload, Payload>>::plan_value(value, demand)
    }
}
impl ClonePlanning<Payload, Option<Payload>> for NativeStoragePolicy {
    fn plan_key(key: &Payload, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        <Policy as ClonePlanning<Payload, Option<Payload>>>::plan_key(key, demand)
    }
    fn plan_value(
        value: &Option<Payload>,
        demand: &mut AllocationDemand,
    ) -> Result<(), PlanningError> {
        <Policy as ClonePlanning<Payload, Option<Payload>>>::plan_value(value, demand)
    }
}
impl AdmittedStoragePolicy for NativeStoragePolicy {
    fn from_admission(mut reservation: AllocationReservation) -> Self {
        let fail_at = match FACTORY_FAULT.with(|mode| mode.replace(0)) {
            0 => None,
            1 => {
                let foreign = FOREIGN_POOL.with(|slot| {
                    slot.borrow()
                        .as_ref()
                        .expect("explicit equal-valued foreign pool")
                        .try_reserve_bytes(reservation.remaining_bytes())
                        .unwrap()
                });
                drop(reservation);
                reservation = foreign;
                None
            }
            2 => {
                drop(
                    reservation
                        .try_partition(1)
                        .expect("nonempty planned policy"),
                );
                None
            }
            3 => panic!("injected actual Storage policy factory panic"),
            4 => Some(1),
            _ => unreachable!("closed test fault"),
        };
        let counters = STORAGE_COUNTERS.with(|slot| {
            Arc::clone(
                slot.borrow()
                    .as_ref()
                    .expect("configured original counters"),
            )
        });
        counters.admissions.fetch_add(1, SeqCst);
        Self(Policy {
            reservation,
            counters,
            copies: 0,
            fail_at,
        })
    }
    fn admission(&self) -> &AllocationReservation {
        &self.0.reservation
    }
}

type NativeStorage = Storage<Payload, Payload, Prepaid<NativeStoragePolicy>>;
type NativeBlock<'a> = mv::storage::Block<'a, Payload, Payload, Prepaid<NativeStoragePolicy>>;

fn put(
    block: &mut NativeBlock<'_>,
    budget: &AllocationBudget,
    order: usize,
    marker: u8,
) -> Option<Payload> {
    let (key, mut value) = input(budget, order);
    value.bytes.fill(marker);
    block
        .try_insert_admitted(key, value)
        .unwrap_or_else(|(_, error)| panic!("original Storage edit refused: {error:?}"))
}
fn marker(value: Option<&Payload>, expected: u8) {
    assert!(
        value
            .expect("retained value")
            .bytes
            .iter()
            .all(|byte| *byte == expected)
    );
}
fn seed(storage: &NativeStorage, budget: &AllocationBudget, marker: u8) {
    storage
        .try_with_admitted_block(|block| {
            assert!(put(block, budget, 7, marker).is_none());
            Ok::<_, ()>(())
        })
        .unwrap();
}

#[test]
fn actual_storage_resets_first_none_and_some_between_blocks_and_aborts_parent() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    let (query7, unused7) = input(&budget, 7);
    let (query8, unused8) = input(&budget, 8);
    drop((unused7, unused8));
    storage
        .try_with_admitted_block(|block| {
            assert!(block.is_empty() && !block.is_dirty());
            assert!(put(block, &budget, 7, 0x11).is_none());
            assert!(block.get_before_block(&query7).is_none());
            drop(put(block, &budget, 7, 0x12));
            assert!(block.get_before_block(&query7).is_none());
            assert!(block.is_dirty());
            Ok::<_, ()>(())
        })
        .unwrap();
    marker(storage.view().get(&7), 0x12);
    storage
        .try_with_admitted_block(|block| {
            assert!(!block.is_dirty());
            marker(block.get_before_block(&query7), 0x12);
            drop(put(block, &budget, 7, 0x21));
            let first = block.get_before_block(&query7).unwrap().pointer();
            drop(put(block, &budget, 7, 0x22));
            assert_eq!(block.get_before_block(&query7).unwrap().pointer(), first);
            marker(block.get_before_block(&query7), 0x12);
            assert!(put(block, &budget, 8, 0x28).is_none());
            drop(put(block, &budget, 8, 0x29));
            assert!(block.get_before_block(&query8).is_none());
            assert_eq!(block.len(), 2);
            Ok::<_, ()>(())
        })
        .unwrap();
    let published = storage.view();
    let pointers = (
        published.get(&7).unwrap().pointer(),
        published.get(&8).unwrap().pointer(),
    );
    let before = budget.reserved_bytes();
    let result = storage.try_with_admitted_block(|block| {
        // Prior first-None and first-Some rows must both be cleared at this block cut.
        marker(block.get_before_block(&query7), 0x22);
        marker(block.get_before_block(&query8), 0x29);
        drop(put(block, &budget, 7, 0x31));
        drop(put(block, &budget, 8, 0x38));
        marker(block.get_before_block(&query7), 0x22);
        marker(block.get_before_block(&query8), 0x29);
        Err::<(), _>("abort parent")
    });
    assert!(matches!(
        result,
        Err(AdmittedBlockError::Callback("abort parent"))
    ));
    assert_eq!(budget.reserved_bytes(), before);
    let after = storage.view();
    assert_eq!(
        (
            after.get(&7).unwrap().pointer(),
            after.get(&8).unwrap().pointer()
        ),
        pointers
    );
    marker(after.get(&7), 0x22);
    marker(after.get(&8), 0x29);
    drop((published, after, query7, query8));
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn actual_storage_joined_refusal_precedes_clone_and_exact_budget_retry_preserves_input() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0x41);
    let old = storage.view();
    let original = old.get(&7).unwrap().pointer();
    storage.try_with_admitted_block(|block| {
        let (key, mut value) = input(&budget, 7);
        value.bytes.fill(0x42);
        let pointers = (key.pointer(), value.pointer());
        let counts = (counters.admissions.load(SeqCst), counters.keys.load(SeqCst), counters.values.load(SeqCst));
        let records = NEXT_RECORD.load(SeqCst);
        let base = budget.reserved_bytes();
        let blocker = budget.try_reserve_bytes(budget.limit_bytes() - base).unwrap();
        let ((key, value), error) = without_allocations(|| block.try_insert_admitted(key, value).err().expect("full original pool must refuse"));
        let AdmittedStorageError::Allocation(AllocationRefusal::Capacity { requested_bytes, .. }) = error else { panic!("original joined capacity refusal"); };
        assert!(requested_bytes > 0);
        assert_eq!((key.pointer(), value.pointer()), pointers);
        assert_eq!(block.get(&7).unwrap().pointer(), original);
        assert!(!block.is_dirty());
        assert_eq!(NEXT_RECORD.load(SeqCst), records);
        assert_eq!((counters.admissions.load(SeqCst), counters.keys.load(SeqCst), counters.values.load(SeqCst)), counts);
        drop(blocker);
        let blocker = budget.try_reserve_bytes(budget.limit_bytes() - base - requested_bytes + 1).unwrap();
        let ((key, value), error) = without_allocations(|| block.try_insert_admitted(key, value).err().expect("one byte below joined demand must refuse"));
        assert!(matches!(error, AdmittedStorageError::Allocation(AllocationRefusal::Capacity { requested_bytes: bytes, .. }) if bytes == requested_bytes));
        assert_eq!((key.pointer(), value.pointer()), pointers);
        assert_eq!(NEXT_RECORD.load(SeqCst), records);
        assert_eq!((counters.admissions.load(SeqCst), counters.keys.load(SeqCst), counters.values.load(SeqCst)), counts);
        drop(blocker);
        let blocker = budget.try_reserve_bytes(budget.limit_bytes() - base - requested_bytes).unwrap();
        let previous = block.try_insert_admitted(key, value).unwrap_or_else(|(_, error)| panic!("exact original joined budget refused: {error:?}"));
        assert_eq!(block.get(&7).unwrap().pointer(), pointers.1);
        marker(block.get(&7), 0x42);
        assert!(block.is_dirty());
        assert_eq!(old.get(&7).unwrap().pointer(), original);
        assert_eq!(storage.view().get(&7).unwrap().pointer(), original);
        drop((previous, blocker));
        Ok::<_, ()>(())
    }).unwrap();
    marker(storage.view().get(&7), 0x42);
    drop(old);
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn actual_storage_old_reader_owns_nested_bytes_and_credits_until_physical_release() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0x51);
    let old = storage.view();
    let original = old.get(&7).unwrap();
    let (id, pointer) = (original.id(), original.pointer());
    storage
        .try_with_admitted_block(|block| {
            drop(put(block, &budget, 7, 0x52));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(old.get(&7).unwrap().pointer(), pointer);
    marker(old.get(&7), 0x51);
    marker(storage.view().get(&7), 0x52);
    assert!(!RECORDS[id].freed.load(SeqCst));
    assert!(!RECORDS[id].refunded.load(SeqCst));
    let before = budget.reserved_bytes();
    without_allocations(|| drop(old));
    assert!(RECORDS[id].freed.load(SeqCst));
    assert!(RECORDS[id].refunded.load(SeqCst));
    assert!(budget.reserved_bytes() < before);
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

struct ReenterStorage {
    storage: Arc<NativeStorage>,
    wakes: AtomicUsize,
    entered: AtomicBool,
}
impl Wake for ReenterStorage {
    fn wake(self: Arc<Self>) {
        let result = self.storage.try_with_admitted_block(|_| {
            self.entered.store(true, SeqCst);
            Err::<(), _>(())
        });
        assert!(
            matches!(result, Err(AdmittedBlockError::Callback(()))),
            "both original writers must be free before refund wake"
        );
        self.wakes.fetch_add(1, SeqCst);
    }
}

#[test]
fn actual_storage_refund_wake_reenters_only_after_both_original_writers_release() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let storage = Arc::new(NativeStorage::try_new_admitted(budget.clone()).unwrap());
    seed(&storage, &budget, 0x61);
    let old = storage.view();
    let id = old.get(&7).unwrap().id();
    storage
        .try_with_admitted_block(|block| {
            drop(put(block, &budget, 7, 0x62));
            Ok::<_, ()>(())
        })
        .unwrap();
    let wake = Arc::new(ReenterStorage {
        storage: Arc::clone(&storage),
        wakes: AtomicUsize::new(0),
        entered: AtomicBool::new(false),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut context = Context::from_waker(&waker);
    let mut wait = None;
    let result = storage.try_with_admitted_block(|block| {
        assert!(matches!(
            storage.try_with_admitted_block(|_| Ok::<_, ()>(())),
            Err(AdmittedBlockError::Admission(
                AdmittedStorageError::Busy { .. }
            ))
        ));
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
            .unwrap();
        let AllocationRefusal::Capacity { release, .. } = budget.try_reserve_bytes(1).unwrap_err()
        else {
            panic!("original pool is full");
        };
        let mut pending = Box::pin(release.wait_for_release());
        assert!(pending.as_mut().poll(&mut context).is_pending());
        wait = Some(pending);
        without_allocations(|| {
            drop(old);
            assert!(RECORDS[id].freed.load(SeqCst));
            assert!(RECORDS[id].refunded.load(SeqCst));
            assert_eq!(wake.wakes.load(SeqCst), 0);
            drop(blocker);
            assert_eq!(wake.wakes.load(SeqCst), 0);
        });
        marker(block.get(&7), 0x62);
        Err::<(), _>(())
    });
    assert!(matches!(result, Err(AdmittedBlockError::Callback(()))));
    assert_eq!(wake.wakes.load(SeqCst), 1);
    assert!(wake.entered.load(SeqCst));
    assert!(
        wait.as_mut()
            .unwrap()
            .as_mut()
            .poll(&mut context)
            .is_ready()
    );
    marker(storage.view().get(&7), 0x62);
    drop((wait, waker, wake));
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

fn expect_factory_refusal(error: AdmittedStorageError, fault: u8) {
    match (fault, error) {
        (1, AdmittedStorageError::PolicyIdentity) => {}
        (
            2,
            AdmittedStorageError::PolicyDemand {
                expected_bytes,
                remaining_bytes,
            },
        ) => {
            assert_eq!(remaining_bytes + 1, expected_bytes);
        }
        (_, error) => panic!("wrong original admission refusal: {error:?}"),
    }
}

#[test]
fn actual_storage_constructor_rejects_foreign_and_short_policy_without_retained_credits() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for fault in [1, 2] {
        reset();
        let budget = AllocationBudget::new(8 << 20);
        let foreign = AllocationBudget::new(budget.limit_bytes());
        let counters = Arc::new(Counters::default());
        let _context = PolicyContext::new(&counters);
        FOREIGN_POOL.with(|slot| assert!(slot.replace(Some(foreign.clone())).is_none()));
        FACTORY_FAULT.with(|mode| mode.set(fault));
        let error = NativeStorage::try_new_admitted(budget.clone())
            .err()
            .expect("invalid policy cannot construct production Storage");
        expect_factory_refusal(error, fault);
        assert_eq!(budget.reserved_bytes(), 0);
        assert_eq!(foreign.reserved_bytes(), 0);
        assert_eq!(counters.keys.load(SeqCst), 0);
        assert_eq!(counters.values.load(SeqCst), 0);
        assert_eq!(NEXT_RECORD.load(SeqCst), 0);
        let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
        seed(&storage, &budget, 0x71);
        marker(storage.view().get(&7), 0x71);
        without_allocations(|| drop(storage));
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn actual_storage_edit_rejects_foreign_and_short_policy_before_cloning_or_mutation() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for fault in [1, 2] {
        reset();
        let budget = AllocationBudget::new(8 << 20);
        let foreign = AllocationBudget::new(budget.limit_bytes());
        let counters = Arc::new(Counters::default());
        let _context = PolicyContext::new(&counters);
        FOREIGN_POOL.with(|slot| assert!(slot.replace(Some(foreign.clone())).is_none()));
        let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
        seed(&storage, &budget, 0x81);
        let published = storage.view();
        let original = published.get(&7).unwrap().pointer();
        storage
            .try_with_admitted_block(|block| {
                let (key, mut value) = input(&budget, 7);
                value.bytes.fill(0x82);
                let pointers = (key.pointer(), value.pointer());
                let before = budget.reserved_bytes();
                let records = NEXT_RECORD.load(SeqCst);
                let copies = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
                FACTORY_FAULT.with(|mode| mode.set(fault));
                let ((key, value), error) = without_allocations(|| {
                    block
                        .try_insert_admitted(key, value)
                        .err()
                        .expect("changed original reservation must refuse before either edit")
                });
                expect_factory_refusal(error, fault);
                assert_eq!((key.pointer(), value.pointer()), pointers);
                assert_eq!(block.get(&7).unwrap().pointer(), original);
                assert!(!block.is_dirty());
                assert_eq!(NEXT_RECORD.load(SeqCst), records);
                assert_eq!(
                    (counters.keys.load(SeqCst), counters.values.load(SeqCst)),
                    copies
                );
                assert_eq!(budget.reserved_bytes(), before);
                assert_eq!(foreign.reserved_bytes(), 0);
                drop(
                    block
                        .try_insert_admitted(key, value)
                        .unwrap_or_else(|(_, error)| {
                            panic!("same inputs must retry against original budget: {error:?}")
                        }),
                );
                marker(block.get(&7), 0x82);
                assert_eq!(published.get(&7).unwrap().pointer(), original);
                Ok::<_, ()>(())
            })
            .unwrap();
        marker(storage.view().get(&7), 0x82);
        drop(published);
        without_allocations(|| drop(storage));
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn actual_storage_summed_startup_and_reset_refusal_preserve_both_committed_images() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    type Current = BptreeMap<Payload, Payload, Prepaid<NativeStoragePolicy>>;
    type Undo = BptreeMap<Payload, Option<Payload>, Prepaid<NativeStoragePolicy>>;
    let initial = Current::node_custody_allocation_demand()
        .unwrap()
        .bytes()
        .checked_add(Undo::node_custody_allocation_demand().unwrap().bytes())
        .unwrap();
    assert!(initial > 0);
    let insufficient = AllocationBudget::new(initial - 1);
    let error = without_allocations(|| NativeStorage::try_new_admitted(insufficient.clone()))
        .err()
        .expect("complete original two-map sum must be admitted first");
    assert!(
        matches!(error, AdmittedStorageError::Allocation(AllocationRefusal::ExceedsLimit {
        requested_bytes, limit_bytes
    }) if requested_bytes == initial && limit_bytes == initial - 1)
    );
    assert_eq!(counters.admissions.load(SeqCst), 0);
    assert_eq!(insufficient.reserved_bytes(), 0);
    let exact = AllocationBudget::new(initial);
    let empty = NativeStorage::try_new_admitted(exact.clone()).unwrap();
    assert_eq!(counters.admissions.load(SeqCst), 2);
    assert_eq!(exact.reserved_bytes(), initial);
    assert!(empty.view().is_empty());
    without_allocations(|| drop(empty));
    assert_eq!(exact.reserved_bytes(), 0);

    let budget = AllocationBudget::new(8 << 20);
    let mut storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0x91);
    storage
        .try_with_admitted_block(|block| {
            drop(put(block, &budget, 7, 0x92));
            assert!(put(block, &budget, 8, 0x98).is_none());
            Ok::<_, ()>(())
        })
        .unwrap();
    let original = {
        let history = storage.history();
        marker(history.get_before_block(&7), 0x91);
        assert!(history.get_before_block(&8).is_none());
        (
            history.current().get(&7).unwrap().pointer(),
            history.current().get(&8).unwrap().pointer(),
            history.get_before_block(&7).unwrap().pointer(),
        )
    };
    let shells = Current::writer_start_allocation_demand()
        .unwrap()
        .bytes()
        .checked_add(Undo::writer_start_allocation_demand().unwrap().bytes())
        .unwrap();
    let occupied = budget.reserved_bytes();
    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - occupied - shells)
        .unwrap();
    let held = budget.reserved_bytes();
    let calls = counters.admissions.load(SeqCst);
    let copies = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
    let result = storage.try_with_admitted_block(|_| -> Result<(), ()> {
        panic!("undo reset refusal must precede the user callback")
    });
    assert!(matches!(result, Err(AdmittedBlockError::Admission(
        AdmittedStorageError::Allocation(AllocationRefusal::Capacity { requested_bytes, .. })
    )) if requested_bytes > 0));
    assert_eq!(
        counters.admissions.load(SeqCst),
        calls + 2,
        "both writer shells admitted; reset policy must not be constructed"
    );
    assert_eq!(
        (counters.keys.load(SeqCst), counters.values.load(SeqCst)),
        copies
    );
    assert_eq!(budget.reserved_bytes(), held);
    {
        let history = storage.history();
        assert_eq!(
            (
                history.current().get(&7).unwrap().pointer(),
                history.current().get(&8).unwrap().pointer(),
                history.get_before_block(&7).unwrap().pointer()
            ),
            original
        );
        marker(history.get_before_block(&7), 0x91);
        assert!(history.get_before_block(&8).is_none());
    }
    drop(blocker);
    let (query7, spare) = input(&budget, 7);
    drop(spare);
    storage
        .try_with_admitted_block(|block| {
            marker(block.get_before_block(&query7), 0x92);
            drop(put(block, &budget, 7, 0x93));
            marker(block.get_before_block(&query7), 0x92);
            Ok::<_, ()>(())
        })
        .unwrap();
    {
        let history = storage.history();
        marker(history.current().get(&7), 0x93);
        marker(history.get_before_block(&7), 0x92);
        assert_eq!(
            history.get_before_block(&8).unwrap().pointer(),
            history.current().get(&8).unwrap().pointer()
        );
    }
    drop(query7);
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn actual_storage_caught_edit_panic_cannot_publish_and_reclaims_private_credits() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for fault in [3, 4] {
        reset();
        let budget = AllocationBudget::new(8 << 20);
        let counters = Arc::new(Counters::default());
        let _context = PolicyContext::new(&counters);
        let mut storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
        seed(&storage, &budget, 0xa1);
        storage
            .try_with_admitted_block(|block| {
                drop(put(block, &budget, 7, 0xa2));
                assert!(put(block, &budget, 8, 0xa8).is_none());
                Ok::<_, ()>(())
            })
            .unwrap();
        let before = {
            let history = storage.history();
            assert!(history.get_before_block(&8).is_none());
            (
                history.current().get(&7).unwrap().pointer(),
                history.current().get(&8).unwrap().pointer(),
                history.get_before_block(&7).unwrap().pointer(),
            )
        };
        let published = storage.view();
        let reserved = budget.reserved_bytes();
        let start = NEXT_RECORD.load(SeqCst);
        let callback_caught = Cell::new(false);
        let callback_returned_ok = Cell::new(false);
        let aggregate = catch_unwind(AssertUnwindSafe(|| {
            storage.try_with_admitted_block(|block| {
                let (key, value) = input(&budget, 7);
                FACTORY_FAULT.with(|mode| mode.set(fault));
                let first = catch_unwind(AssertUnwindSafe(|| {
                    let _ = block.try_insert_admitted(key, value);
                }));
                assert!(
                    first.is_err(),
                    "real policy/clone injection must fail inside edit"
                );
                callback_caught.set(true);
                callback_returned_ok.set(true);
                Ok::<_, ()>(())
            })
        }));
        assert!(callback_caught.get() && callback_returned_ok.get());
        assert!(
            aggregate.is_err(),
            "aggregate must refuse publication after a caught edit panic"
        );
        assert_eq!(budget.reserved_bytes(), reserved);
        reclaimed_since(start);
        assert_eq!(
            (
                published.get(&7).unwrap().pointer(),
                published.get(&8).unwrap().pointer()
            ),
            (before.0, before.1)
        );
        marker(published.get(&7), 0xa2);
        marker(published.get(&8), 0xa8);
        drop(published);
        {
            let history = storage.history();
            assert_eq!(
                (
                    history.current().get(&7).unwrap().pointer(),
                    history.current().get(&8).unwrap().pointer(),
                    history.get_before_block(&7).unwrap().pointer()
                ),
                before
            );
            marker(history.get_before_block(&7), 0xa1);
            assert!(history.get_before_block(&8).is_none());
        }
        let retry = storage.try_with_admitted_block(|_| -> Result<(), ()> {
            panic!("poisoned original Storage must not invoke a new callback")
        });
        assert!(matches!(
            retry,
            Err(AdmittedBlockError::Admission(
                AdmittedStorageError::Poisoned { .. }
            ))
        ));
        without_allocations(|| drop(storage));
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

type NativeTransaction<'a> =
    mv::storage::Transaction<'a, Payload, Payload, Prepaid<NativeStoragePolicy>>;

fn transaction_put(
    transaction: &mut NativeTransaction<'_>,
    budget: &AllocationBudget,
    order: usize,
    marker: u8,
) -> Option<Payload> {
    let (key, mut value) = input(budget, order);
    value.bytes.fill(marker);
    transaction
        .try_insert_admitted(key, value)
        .unwrap_or_else(|(_, error)| panic!("original Transaction edit refused: {error:?}"))
}

#[test]
fn actual_transaction_joined_touch_and_pair_refusal_preserves_inputs_for_exact_retry() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0xb1);
    let old = storage.view();
    let original = old.get(&7).unwrap().pointer();
    storage.try_with_admitted_block(|block| {
        for order in [7, 9] {
            let mut transaction = without_allocations(|| block.try_transaction_admitted()).unwrap();
            let (key, mut value) = input(&budget, order);
            value.bytes.fill(0xb2);
            let input_pointers = (key.pointer(), value.pointer());
            let counts = (counters.admissions.load(SeqCst), counters.keys.load(SeqCst), counters.values.load(SeqCst));
            let records = NEXT_RECORD.load(SeqCst);
            let held = budget.reserved_bytes();
            let blocker = budget.try_reserve_bytes(budget.limit_bytes() - held).unwrap();
            let ((key, value), error) = without_allocations(|| transaction.try_insert_admitted(key, value).err().expect("complete pair plus first touch must refuse"));
            let AdmittedStorageError::Allocation(AllocationRefusal::Capacity { requested_bytes, .. }) = error else { panic!("original joined capacity refusal"); };
            assert!(requested_bytes > 0);
            assert_eq!((key.pointer(), value.pointer()), input_pointers);
            assert_eq!(transaction.touched_entries().len(), 0);
            assert!(!transaction.is_dirty());
            assert_eq!(NEXT_RECORD.load(SeqCst), records);
            assert_eq!((counters.admissions.load(SeqCst), counters.keys.load(SeqCst), counters.values.load(SeqCst)), counts);
            assert_eq!(transaction.get(&7).unwrap().pointer(), original);
            drop(blocker);
            let blocker = budget.try_reserve_bytes(budget.limit_bytes() - held - requested_bytes + 1).unwrap();
            let ((key, value), error) = without_allocations(|| transaction.try_insert_admitted(key, value).err().expect("one byte below the joined touch and pair demand must refuse"));
            assert!(matches!(error, AdmittedStorageError::Allocation(AllocationRefusal::Capacity { requested_bytes: actual, .. }) if actual == requested_bytes));
            assert_eq!((key.pointer(), value.pointer()), input_pointers);
            assert_eq!(transaction.touched_entries().len(), 0);
            assert_eq!(NEXT_RECORD.load(SeqCst), records);
            assert_eq!((counters.admissions.load(SeqCst), counters.keys.load(SeqCst), counters.values.load(SeqCst)), counts);
            drop(blocker);
            let blocker = budget.try_reserve_bytes(budget.limit_bytes() - held - requested_bytes).unwrap();
            let previous = transaction.try_insert_admitted(key, value).unwrap_or_else(|(_, error)| panic!("exact original joined budget refused: {error:?}"));
            assert_eq!(transaction.get(&order).unwrap().pointer(), input_pointers.1);
            assert!(transaction.is_dirty());
            without_allocations(|| {
                let mut touches = transaction.touched_entries();
                assert_eq!(touches.len(), 1);
                let row = touches.next().unwrap();
                assert_eq!(row.key.order, order);
                assert_ne!(row.key.pointer(), input_pointers.0);
                assert_eq!(row.after.unwrap().pointer(), input_pointers.1);
                assert_eq!(row.before.map(Payload::pointer), if order == 7 { Some(original) } else { None });
                assert!(touches.next().is_none());
            });
            assert_eq!(old.get(&7).unwrap().pointer(), original);
            drop((previous, blocker));
            // Abort makes the next case start at the same actual parent roots.
            without_allocations(|| drop(transaction));
            assert_eq!(block.get(&7).unwrap().pointer(), original);
            assert!(block.get(&9).is_none());
            assert!(!block.is_dirty());
        }
        Ok::<_, ()>(())
    }).unwrap();
    assert_eq!(storage.view().get(&7).unwrap().pointer(), original);
    drop(old);
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn actual_transaction_ordered_unique_touches_preserve_noop_and_sibling_preimages() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let mut storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0xc1);
    let old = storage.view();
    let old_id = old.get(&7).unwrap().id();
    let old_pointer = old.get(&7).unwrap().pointer();
    let (query7, spare7) = input(&budget, 7);
    let (query2, spare2) = input(&budget, 2);
    drop((spare7, spare2));
    storage
        .try_with_admitted_block(|block| {
            let mut transaction = block.try_transaction_admitted().unwrap();
            assert!(transaction.touched_entries().next().is_none());
            assert!(transaction_put(&mut transaction, &budget, 9, 0xc9).is_none());
            drop(transaction_put(&mut transaction, &budget, 7, 0xc1));
            let first_touch = transaction
                .touched_entries()
                .find(|row| row.key.order == 7)
                .unwrap()
                .key
                .pointer();
            let first_block_preimage = transaction.get_before_block(&query7).unwrap().pointer();
            marker(transaction.get_before_block(&query7), 0xc1);
            // Equal value bytes still represent an explicit first touch.
            drop(transaction_put(&mut transaction, &budget, 7, 0xc1));
            assert_eq!(
                transaction.get_before_block(&query7).unwrap().pointer(),
                first_block_preimage
            );
            let undo_copy_start = NEXT_RECORD.load(SeqCst);
            // A new undo key checkpoints the shared leaf, copying its retained Some rows.
            assert!(transaction_put(&mut transaction, &budget, 2, 0xc2).is_none());
            without_allocations(|| {
                let mut rows = transaction.touched_entries();
                assert_eq!(rows.len(), 3);
                let two = rows.next().unwrap();
                assert_eq!(two.key.order, 2);
                assert!(two.before.is_none());
                marker(two.after, 0xc2);
                let nine = rows.next_back().unwrap();
                assert_eq!(nine.key.order, 9);
                assert!(nine.before.is_none());
                marker(nine.after, 0xc9);
                let seven = rows.next().unwrap();
                assert_eq!(seven.key.order, 7);
                assert_eq!(seven.key.pointer(), first_touch);
                assert_eq!(seven.before.unwrap().pointer(), old_pointer);
                marker(seven.before, 0xc1);
                marker(seven.after, 0xc1);
                assert_eq!(rows.len(), 0);
            });
            assert_eq!(
                transaction
                    .get_before_transaction(&query7)
                    .unwrap()
                    .pointer(),
                old_pointer
            );
            let copied_preimage = transaction.get_before_block(&query7).unwrap();
            marker(Some(copied_preimage), 0xc1);
            assert_ne!(copied_preimage.pointer(), first_block_preimage);
            assert!(copied_preimage.id() >= undo_copy_start);
            assert_eq!(
                RECORDS[copied_preimage.id()].pointer.load(SeqCst),
                copied_preimage.pointer()
            );
            assert!(!RECORDS[copied_preimage.id()].freed.load(SeqCst));
            assert!(!RECORDS[copied_preimage.id()].refunded.load(SeqCst));
            assert!(transaction.get_before_block(&query2).is_none());
            without_allocations(|| transaction.apply());
            let parent_pointer = block.get(&7).unwrap().pointer();
            let parent_undo = block.get_before_block(&query7).unwrap().pointer();
            let held = budget.reserved_bytes();
            {
                let mut sibling = block.try_transaction_admitted().unwrap();
                assert_eq!(sibling.touched_entries().len(), 0);
                assert_eq!(
                    sibling.get_before_transaction(&query7).unwrap().pointer(),
                    parent_pointer
                );
                assert_eq!(
                    sibling.get_before_block(&query7).unwrap().pointer(),
                    parent_undo
                );
                drop(transaction_put(&mut sibling, &budget, 7, 0xd1));
                assert!(transaction_put(&mut sibling, &budget, 8, 0xd8).is_none());
                assert_eq!(sibling.touched_entries().len(), 2);
                without_allocations(|| drop(sibling));
            }
            assert_eq!(budget.reserved_bytes(), held);
            assert_eq!(block.get(&7).unwrap().pointer(), parent_pointer);
            assert_eq!(
                block.get_before_block(&query7).unwrap().pointer(),
                parent_undo
            );
            assert!(block.get(&8).is_none());
            let mut sibling = block.try_transaction_admitted().unwrap();
            assert_eq!(sibling.touched_entries().len(), 0);
            drop(transaction_put(&mut sibling, &budget, 7, 0xc7));
            drop(transaction_put(&mut sibling, &budget, 2, 0xc3));
            assert_eq!(
                sibling.get_before_transaction(&query7).unwrap().pointer(),
                parent_pointer
            );
            assert_eq!(
                sibling.get_before_block(&query7).unwrap().pointer(),
                parent_undo
            );
            assert!(sibling.get_before_block(&query2).is_none());
            without_allocations(|| sibling.apply());
            assert_eq!(
                block.get_before_block(&query7).unwrap().pointer(),
                parent_undo
            );
            assert!(block.get_before_block(&query2).is_none());
            assert!(block.is_dirty());
            Ok::<_, ()>(())
        })
        .unwrap();
    marker(storage.view().get(&7), 0xc7);
    marker(storage.view().get(&2), 0xc3);
    assert!(storage.view().get(&8).is_none());
    assert_eq!(old.get(&7).unwrap().pointer(), old_pointer);
    assert!(!RECORDS[old_id].freed.load(SeqCst));
    drop(old);
    {
        let history = storage.history();
        marker(history.get_before_block(&7), 0xc1);
        assert!(history.get_before_block(&2).is_none());
        assert!(history.get_before_block(&9).is_none());
    }
    drop((query7, query2));
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn actual_transaction_full_budget_abort_restores_parent_and_outer_abort_preserves_readers() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(16 << 20);
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0xe1);
    let old = storage.view();
    let original = old.get(&7).unwrap().pointer();
    let (query7, spare7) = input(&budget, 7);
    let (query8, spare8) = input(&budget, 8);
    drop((spare7, spare8));
    let baseline = budget.reserved_bytes();
    let result = storage.try_with_admitted_block(|block| {
        drop(put(block, &budget, 7, 0xe2));
        assert!(put(block, &budget, 8, 0xe8).is_none());
        let parent = (
            block.get(&7).unwrap().pointer(),
            block.get(&8).unwrap().pointer(),
            block.get_before_block(&query7).unwrap().pointer(),
        );
        let held = budget.reserved_bytes();
        let records = NEXT_RECORD.load(SeqCst);
        let mut transaction = without_allocations(|| block.try_transaction_admitted()).unwrap();
        for order in (0..40).rev() {
            drop(transaction_put(&mut transaction, &budget, order, 0xef));
        }
        assert_eq!(transaction.len(), 40);
        assert_eq!(transaction.touched_entries().len(), 40);
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
            .unwrap();
        assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
        without_allocations(|| drop(transaction));
        assert_eq!(budget.reserved_bytes(), held + blocker.remaining_bytes());
        reclaimed_since(records);
        assert_eq!(
            (
                block.get(&7).unwrap().pointer(),
                block.get(&8).unwrap().pointer(),
                block.get_before_block(&query7).unwrap().pointer()
            ),
            parent
        );
        assert_eq!(block.len(), 2);
        assert!(block.get_before_block(&query8).is_none());
        assert!(block.is_dirty());
        drop(blocker);
        let mut applied = block.try_transaction_admitted().unwrap();
        drop(transaction_put(&mut applied, &budget, 7, 0xe3));
        assert_eq!(
            applied.get_before_block(&query7).unwrap().pointer(),
            parent.2
        );
        let undo_copy_start = NEXT_RECORD.load(SeqCst);
        assert!(transaction_put(&mut applied, &budget, 9, 0xe9).is_none());
        let copied_preimage = applied.get_before_block(&query7).unwrap();
        marker(Some(copied_preimage), 0xe1);
        assert_ne!(copied_preimage.pointer(), parent.2);
        assert!(copied_preimage.id() >= undo_copy_start);
        assert_eq!(
            RECORDS[copied_preimage.id()].pointer.load(SeqCst),
            copied_preimage.pointer()
        );
        assert!(!RECORDS[copied_preimage.id()].freed.load(SeqCst));
        assert!(!RECORDS[copied_preimage.id()].refunded.load(SeqCst));
        let applied_undo = copied_preimage.pointer();
        without_allocations(|| applied.apply());
        marker(block.get(&7), 0xe3);
        marker(block.get(&9), 0xe9);
        assert_eq!(
            block.get_before_block(&query7).unwrap().pointer(),
            applied_undo
        );
        marker(block.get_before_block(&query7), 0xe1);
        Err::<(), _>("abort original parent after applied child")
    });
    assert!(matches!(
        result,
        Err(AdmittedBlockError::Callback(
            "abort original parent after applied child"
        ))
    ));
    assert_eq!(budget.reserved_bytes(), baseline);
    assert_eq!(storage.view().get(&7).unwrap().pointer(), original);
    assert_eq!(old.get(&7).unwrap().pointer(), original);
    assert_eq!(storage.view().len(), 1);
    drop((old, query7, query8));
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn actual_transaction_caught_touch_and_pair_copy_panics_cannot_apply_or_publish() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for (fault, already_touched) in [(3, false), (4, false), (4, true)] {
        reset();
        let budget = AllocationBudget::new(8 << 20);
        let counters = Arc::new(Counters::default());
        let _context = PolicyContext::new(&counters);
        let mut storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
        seed(&storage, &budget, 0xf1);
        storage
            .try_with_admitted_block(|block| {
                drop(put(block, &budget, 7, 0xf2));
                assert!(put(block, &budget, 8, 0xf8).is_none());
                Ok::<_, ()>(())
            })
            .unwrap();
        let before = {
            let history = storage.history();
            (
                history.current().get(&7).unwrap().pointer(),
                history.current().get(&8).unwrap().pointer(),
                history.get_before_block(&7).unwrap().pointer(),
            )
        };
        let old = storage.view();
        let held = budget.reserved_bytes();
        let records = NEXT_RECORD.load(SeqCst);
        let callback_returned_ok = Cell::new(false);
        let aggregate = catch_unwind(AssertUnwindSafe(|| {
            storage.try_with_admitted_block(|block| {
                let mut transaction = block.try_transaction_admitted().unwrap();
                if already_touched {
                    drop(transaction_put(&mut transaction, &budget, 7, 0xf3));
                    assert_eq!(transaction.touched_entries().len(), 1);
                } else {
                    assert_eq!(transaction.touched_entries().len(), 0);
                }
                let (key, value) = input(&budget, 7);
                let copies = counters.keys.load(SeqCst);
                let partial = NEXT_RECORD.load(SeqCst);
                FACTORY_FAULT.with(|mode| mode.set(fault));
                let edit = catch_unwind(AssertUnwindSafe(|| {
                    let _ = transaction.try_insert_admitted(key, value);
                }));
                assert!(
                    edit.is_err(),
                    "actual policy factory/touch/pair copy must panic"
                );
                assert_eq!(FACTORY_FAULT.with(Cell::get), 0);
                if fault == 3 {
                    assert_eq!(counters.keys.load(SeqCst), copies);
                    assert_eq!(NEXT_RECORD.load(SeqCst), partial);
                } else {
                    assert!(counters.keys.load(SeqCst) > copies);
                    assert!(
                        NEXT_RECORD.load(SeqCst) > partial,
                        "copy panic follows an actual nested allocation"
                    );
                }
                assert!(
                    catch_unwind(AssertUnwindSafe(|| transaction.apply())).is_err(),
                    "caught edit cannot apply either checkpoint"
                );
                callback_returned_ok.set(true);
                Ok::<_, ()>(())
            })
        }));
        assert!(callback_returned_ok.get());
        assert!(
            aggregate.is_err(),
            "original parent must refuse publication after caught child failure"
        );
        assert_eq!(budget.reserved_bytes(), held);
        reclaimed_since(records);
        assert_eq!(
            (
                old.get(&7).unwrap().pointer(),
                old.get(&8).unwrap().pointer()
            ),
            (before.0, before.1)
        );
        drop(old);
        {
            let history = storage.history();
            assert_eq!(
                (
                    history.current().get(&7).unwrap().pointer(),
                    history.current().get(&8).unwrap().pointer(),
                    history.get_before_block(&7).unwrap().pointer()
                ),
                before
            );
            assert!(history.get_before_block(&8).is_none());
        }
        assert!(matches!(
            storage.try_with_admitted_block(|_| -> Result<(), ()> {
                panic!("poisoned parent cannot admit another callback")
            }),
            Err(AdmittedBlockError::Admission(
                AdmittedStorageError::Poisoned { .. }
            ))
        ));
        without_allocations(|| drop(storage));
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn actual_transaction_touch_destructor_panic_cannot_apply_or_publish() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for apply in [false, true] {
        reset();
        let budget = AllocationBudget::new(8 << 20);
        let counters = Arc::new(Counters::default());
        let _context = PolicyContext::new(&counters);
        let mut storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
        seed(&storage, &budget, 0x91);
        storage
            .try_with_admitted_block(|block| {
                drop(put(block, &budget, 7, 0x92));
                assert!(put(block, &budget, 8, 0x98).is_none());
                Ok::<_, ()>(())
            })
            .unwrap();
        let before = {
            let history = storage.history();
            (
                history.current().get(&7).unwrap().pointer(),
                history.current().get(&8).unwrap().pointer(),
                history.get_before_block(&7).unwrap().pointer(),
            )
        };
        let old = storage.view();
        let held = budget.reserved_bytes();
        let records = NEXT_RECORD.load(SeqCst);
        let callback_returned_ok = Cell::new(false);
        let aggregate = catch_unwind(AssertUnwindSafe(|| {
            storage.try_with_admitted_block(|block| {
                let mut transaction = block.try_transaction_admitted().unwrap();
                drop(transaction_put(&mut transaction, &budget, 7, 0x93));
                assert!(transaction_put(&mut transaction, &budget, 9, 0x99).is_none());
                let touch_id = transaction.touched_entries().next().unwrap().key.id();
                assert!(!RECORDS[touch_id].freed.load(SeqCst));
                PANIC_CHARGE.store(touch_id, SeqCst);
                let cleanup = catch_unwind(AssertUnwindSafe(|| {
                    if apply {
                        transaction.apply();
                    } else {
                        drop(transaction);
                    }
                }));
                assert!(
                    cleanup.is_err(),
                    "actual touched-key destructor must run before settlement"
                );
                assert_eq!(PANIC_CHARGE.load(SeqCst), usize::MAX);
                assert!(RECORDS[touch_id].freed.load(SeqCst));
                assert!(RECORDS[touch_id].refunded.load(SeqCst));
                callback_returned_ok.set(true);
                Ok::<_, ()>(())
            })
        }));
        assert!(callback_returned_ok.get());
        assert!(
            aggregate.is_err(),
            "caught cleanup cannot publish a partially settled transaction"
        );
        assert_eq!(budget.reserved_bytes(), held);
        reclaimed_since(records);
        assert_eq!(
            (
                old.get(&7).unwrap().pointer(),
                old.get(&8).unwrap().pointer()
            ),
            (before.0, before.1)
        );
        drop(old);
        {
            let history = storage.history();
            assert_eq!(
                (
                    history.current().get(&7).unwrap().pointer(),
                    history.current().get(&8).unwrap().pointer(),
                    history.get_before_block(&7).unwrap().pointer()
                ),
                before
            );
            assert!(history.get_before_block(&8).is_none());
        }
        assert!(matches!(
            storage.try_with_admitted_block(|_| -> Result<(), ()> {
                panic!("cleanup-poisoned parent cannot admit another callback")
            }),
            Err(AdmittedBlockError::Admission(
                AdmittedStorageError::Poisoned { .. }
            ))
        ));
        without_allocations(|| drop(storage));
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

fn removal_key(budget: &AllocationBudget, order: usize) -> Payload {
    let (key, spare) = input(budget, order);
    drop(spare);
    key
}
fn block_remove(
    block: &mut NativeBlock<'_>,
    budget: &AllocationBudget,
    order: usize,
) -> Option<Payload> {
    block
        .try_remove_admitted(removal_key(budget, order))
        .unwrap_or_else(|(_, error)| panic!("original Block removal refused: {error:?}"))
}
fn transaction_remove(
    transaction: &mut NativeTransaction<'_>,
    budget: &AllocationBudget,
    order: usize,
) -> Option<Payload> {
    transaction
        .try_remove_admitted(removal_key(budget, order))
        .unwrap_or_else(|(_, error)| panic!("original Transaction removal refused: {error:?}"))
}

#[test]
fn actual_block_removal_preserves_first_preimages_and_absent_dirty_semantics() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let mut storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0x11);
    let old = storage.view();
    let original = old.get(&7).unwrap().pointer();
    let old_id = old.get(&7).unwrap().id();
    let query7 = removal_key(&budget, 7);
    let query8 = removal_key(&budget, 8);
    let query9 = removal_key(&budget, 9);
    storage
        .try_with_admitted_block(|block| {
            for _ in 0..2 {
                let previous = block.get(&7).unwrap().pointer();
                let records = NEXT_RECORD.load(SeqCst);
                assert!(block_remove(block, &budget, 9).is_none());
                assert!(!block.is_dirty());
                // Canonical absent deletion still clones its current leaf.
                // Logical cleanliness does not erase that original paid owner.
                let current = block.get(&7).unwrap();
                marker(Some(current), 0x11);
                assert_ne!(current.pointer(), previous);
                assert!(current.id() >= records);
                assert_eq!(
                    RECORDS[current.id()].pointer.load(SeqCst),
                    current.pointer()
                );
                assert!(!RECORDS[current.id()].freed.load(SeqCst));
                assert!(!RECORDS[current.id()].refunded.load(SeqCst));
                assert_eq!(old.get(&7).unwrap().pointer(), original);
                assert!(!RECORDS[old_id].freed.load(SeqCst));
            }
            let removed = block_remove(block, &budget, 7).unwrap();
            marker(Some(&removed), 0x11);
            drop(removed);
            assert!(block.is_dirty());
            assert!(block.get(&7).is_none());
            marker(block.get_before_block(&query7), 0x11);
            let first = block.get_before_block(&query7).unwrap().pointer();
            assert!(block_remove(block, &budget, 7).is_none());
            assert_eq!(block.get_before_block(&query7).unwrap().pointer(), first);
            assert!(put(block, &budget, 7, 0x12).is_none());
            assert_eq!(block.get_before_block(&query7).unwrap().pointer(), first);
            let removed = block_remove(block, &budget, 7).unwrap();
            marker(Some(&removed), 0x12);
            drop(removed);
            assert_eq!(block.get_before_block(&query7).unwrap().pointer(), first);
            assert!(put(block, &budget, 8, 0x18).is_none());
            assert!(block.get_before_block(&query8).is_none());
            drop(block_remove(block, &budget, 8));
            assert!(block.get_before_block(&query8).is_none());
            assert!(put(block, &budget, 7, 0x13).is_none());
            assert!(put(block, &budget, 9, 0x19).is_none());
            marker(block.get_before_block(&query7), 0x11);
            assert!(block.get_before_block(&query9).is_none());
            Ok::<_, ()>(())
        })
        .unwrap();
    marker(storage.view().get(&7), 0x13);
    marker(storage.view().get(&9), 0x19);
    assert!(storage.view().get(&8).is_none());
    assert_eq!(old.get(&7).unwrap().pointer(), original);
    assert!(!RECORDS[old_id].freed.load(SeqCst));
    drop(old);
    {
        let history = storage.history();
        marker(history.get_before_block(&7), 0x11);
        assert!(history.get_before_block(&8).is_none());
        assert!(history.get_before_block(&9).is_none());
    }
    drop((query7, query8, query9));
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn actual_transaction_removal_orders_explicit_absence_and_sibling_preimages() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let mut storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0x21);
    let old = storage.view();
    let original = old.get(&7).unwrap().pointer();
    let query7 = removal_key(&budget, 7);
    let query9 = removal_key(&budget, 9);
    storage
        .try_with_admitted_block(|block| {
            let mut transaction = block.try_transaction_admitted().unwrap();
            assert!(transaction_remove(&mut transaction, &budget, 9).is_none());
            assert!(!transaction.is_dirty());
            let absent_touch = transaction.touched_entries().next().unwrap().key.pointer();
            assert!(transaction_remove(&mut transaction, &budget, 9).is_none());
            assert!(!transaction.is_dirty());
            assert_eq!(transaction.touched_entries().len(), 1);
            assert_eq!(
                transaction.touched_entries().next().unwrap().key.pointer(),
                absent_touch
            );
            let removed = transaction_remove(&mut transaction, &budget, 7).unwrap();
            marker(Some(&removed), 0x21);
            drop(removed);
            assert!(transaction.is_dirty());
            assert!(transaction_put(&mut transaction, &budget, 7, 0x22).is_none());
            drop(transaction_remove(&mut transaction, &budget, 7));
            assert!(transaction_put(&mut transaction, &budget, 7, 0x23).is_none());
            assert!(transaction_put(&mut transaction, &budget, 2, 0x22).is_none());
            drop(transaction_remove(&mut transaction, &budget, 2));
            without_allocations(|| {
                let mut rows = transaction.touched_entries();
                assert_eq!(rows.len(), 3);
                let two = rows.next().unwrap();
                assert_eq!(two.key.order, 2);
                assert!(two.before.is_none() && two.after.is_none());
                let nine = rows.next_back().unwrap();
                assert_eq!(nine.key.order, 9);
                assert_eq!(nine.key.pointer(), absent_touch);
                assert!(nine.before.is_none() && nine.after.is_none());
                let seven = rows.next().unwrap();
                assert_eq!(seven.key.order, 7);
                assert_eq!(seven.before.unwrap().pointer(), original);
                marker(seven.before, 0x21);
                marker(seven.after, 0x23);
                assert!(rows.next().is_none());
            });
            marker(transaction.get_before_block(&query7), 0x21);
            assert!(transaction.get_before_block(&query9).is_none());
            without_allocations(|| transaction.apply());
            let parent = (
                block.get(&7).unwrap().pointer(),
                block.get_before_block(&query7).unwrap().pointer(),
            );
            let held = budget.reserved_bytes();
            {
                let mut sibling = block.try_transaction_admitted().unwrap();
                assert_eq!(sibling.touched_entries().len(), 0);
                drop(transaction_remove(&mut sibling, &budget, 7));
                assert!(transaction_put(&mut sibling, &budget, 8, 0x28).is_none());
                drop(transaction_remove(&mut sibling, &budget, 8));
                assert_eq!(sibling.touched_entries().len(), 2);
                without_allocations(|| drop(sibling));
            }
            assert_eq!(budget.reserved_bytes(), held);
            assert_eq!(
                (
                    block.get(&7).unwrap().pointer(),
                    block.get_before_block(&query7).unwrap().pointer()
                ),
                parent
            );
            assert!(block.get(&8).is_none());
            let mut sibling = block.try_transaction_admitted().unwrap();
            assert_eq!(
                sibling.get_before_transaction(&query7).unwrap().pointer(),
                parent.0
            );
            drop(transaction_remove(&mut sibling, &budget, 7));
            assert!(transaction_remove(&mut sibling, &budget, 9).is_none());
            assert!(transaction_put(&mut sibling, &budget, 9, 0x29).is_none());
            assert_eq!(
                sibling.get_before_block(&query7).unwrap().pointer(),
                parent.1
            );
            assert!(sibling.get_before_block(&query9).is_none());
            without_allocations(|| sibling.apply());
            assert!(block.get(&7).is_none());
            marker(block.get(&9), 0x29);
            assert_eq!(block.get_before_block(&query7).unwrap().pointer(), parent.1);
            Ok::<_, ()>(())
        })
        .unwrap();
    assert!(storage.view().get(&7).is_none());
    marker(storage.view().get(&9), 0x29);
    assert_eq!(old.get(&7).unwrap().pointer(), original);
    drop(old);
    {
        let history = storage.history();
        marker(history.get_before_block(&7), 0x21);
        assert!(history.get_before_block(&2).is_none());
        assert!(history.get_before_block(&9).is_none());
    }
    drop((query7, query9));
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn actual_block_removal_refusal_preserves_exact_query_for_complete_budget_retry() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0x31);
    let old = storage.view();
    let original = old.get(&7).unwrap().pointer();
    for order in [7, 9] {
        let baseline = budget.reserved_bytes();
        let result = storage.try_with_admitted_block(|block| {
            let key = removal_key(&budget, order);
            let query = (key.pointer(), key.id());
            let copies = (counters.admissions.load(SeqCst), counters.keys.load(SeqCst), counters.values.load(SeqCst));
            let records = NEXT_RECORD.load(SeqCst);
            let held = budget.reserved_bytes();
            let blocker = budget.try_reserve_bytes(budget.limit_bytes() - held).unwrap();
            let (key, error) = without_allocations(|| block.try_remove_admitted(key).err().expect("whole original remove pair must refuse"));
            let AdmittedStorageError::Allocation(AllocationRefusal::Capacity { requested_bytes, .. }) = error else { panic!("original remove capacity refusal"); };
            assert!(requested_bytes > 0);
            assert_eq!((key.pointer(), key.id()), query);
            assert_eq!(block.get(&7).unwrap().pointer(), original);
            assert!(!block.is_dirty());
            assert_eq!(NEXT_RECORD.load(SeqCst), records);
            assert_eq!((counters.admissions.load(SeqCst), counters.keys.load(SeqCst), counters.values.load(SeqCst)), copies);
            drop(blocker);
            let blocker = budget.try_reserve_bytes(budget.limit_bytes() - held - requested_bytes + 1).unwrap();
            let (key, error) = without_allocations(|| block.try_remove_admitted(key).err().expect("one byte below complete remove demand"));
            assert!(matches!(error, AdmittedStorageError::Allocation(AllocationRefusal::Capacity { requested_bytes: n, .. }) if n == requested_bytes));
            assert_eq!((key.pointer(), key.id()), query);
            assert_eq!(NEXT_RECORD.load(SeqCst), records);
            assert_eq!((counters.admissions.load(SeqCst), counters.keys.load(SeqCst), counters.values.load(SeqCst)), copies);
            drop(blocker);
            let blocker = budget.try_reserve_bytes(budget.limit_bytes() - held - requested_bytes).unwrap();
            let removed = block.try_remove_admitted(key).unwrap_or_else(|(_, error)| panic!("exact remove budget refused: {error:?}"));
            assert_eq!(removed.is_some(), order == 7);
            if let Some(value) = &removed { marker(Some(value), 0x31); }
            assert_eq!(block.is_dirty(), order == 7);
            assert!(block.get(&order).is_none());
            assert!(RECORDS[query.1].freed.load(SeqCst));
            assert!(RECORDS[query.1].refunded.load(SeqCst));
            drop((removed, blocker));
            Err::<(), _>("abort admitted removal")
        });
        assert!(matches!(
            result,
            Err(AdmittedBlockError::Callback("abort admitted removal"))
        ));
        assert_eq!(budget.reserved_bytes(), baseline);
        assert_eq!(storage.view().get(&7).unwrap().pointer(), original);
        assert_eq!(old.get(&7).unwrap().pointer(), original);
    }
    drop(old);
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn actual_transaction_removal_refusal_joins_touch_and_pair_before_exact_query_retry() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0x41);
    let old = storage.view();
    let original = old.get(&7).unwrap().pointer();
    storage.try_with_admitted_block(|block| {
        for order in [7, 9] {
            let parent_credits = budget.reserved_bytes();
            let mut transaction = without_allocations(|| block.try_transaction_admitted()).unwrap();
            let key = removal_key(&budget, order);
            let query = (key.pointer(), key.id());
            let copies = (counters.admissions.load(SeqCst), counters.keys.load(SeqCst), counters.values.load(SeqCst));
            let records = NEXT_RECORD.load(SeqCst);
            let held = budget.reserved_bytes();
            let blocker = budget.try_reserve_bytes(budget.limit_bytes() - held).unwrap();
            let (key, error) = without_allocations(|| transaction.try_remove_admitted(key).err().expect("joined remove plus touch demand must refuse"));
            let AdmittedStorageError::Allocation(AllocationRefusal::Capacity { requested_bytes, .. }) = error else { panic!("original remove plus touch capacity refusal"); };
            assert!(requested_bytes > 0);
            assert_eq!((key.pointer(), key.id()), query);
            assert_eq!(transaction.touched_entries().len(), 0);
            assert!(!transaction.is_dirty());
            assert_eq!(transaction.get(&7).unwrap().pointer(), original);
            assert_eq!(NEXT_RECORD.load(SeqCst), records);
            assert_eq!((counters.admissions.load(SeqCst), counters.keys.load(SeqCst), counters.values.load(SeqCst)), copies);
            drop(blocker);
            let blocker = budget.try_reserve_bytes(budget.limit_bytes() - held - requested_bytes + 1).unwrap();
            let (key, error) = without_allocations(|| transaction.try_remove_admitted(key).err().expect("one byte below joined remove plus touch demand"));
            assert!(matches!(error, AdmittedStorageError::Allocation(AllocationRefusal::Capacity { requested_bytes: n, .. }) if n == requested_bytes));
            assert_eq!((key.pointer(), key.id()), query);
            assert_eq!(transaction.touched_entries().len(), 0);
            assert_eq!(NEXT_RECORD.load(SeqCst), records);
            assert_eq!((counters.admissions.load(SeqCst), counters.keys.load(SeqCst), counters.values.load(SeqCst)), copies);
            drop(blocker);
            let blocker = budget.try_reserve_bytes(budget.limit_bytes() - held - requested_bytes).unwrap();
            let removed = transaction.try_remove_admitted(key).unwrap_or_else(|(_, error)| panic!("exact joined remove plus touch budget refused: {error:?}"));
            assert_eq!(removed.is_some(), order == 7);
            if let Some(value) = &removed { marker(Some(value), 0x41); }
            assert_eq!(transaction.is_dirty(), order == 7);
            without_allocations(|| {
                let mut rows = transaction.touched_entries();
                assert_eq!(rows.len(), 1);
                let row = rows.next().unwrap();
                assert_eq!(row.key.order, order);
                assert_ne!(row.key.id(), query.1);
                assert_eq!(row.before.map(Payload::pointer), if order == 7 { Some(original) } else { None });
                assert!(row.after.is_none());
                assert!(rows.next().is_none());
            });
            assert!(RECORDS[query.1].freed.load(SeqCst));
            assert!(RECORDS[query.1].refunded.load(SeqCst));
            drop((removed, blocker));
            without_allocations(|| drop(transaction));
            assert_eq!(budget.reserved_bytes(), parent_credits);
            assert_eq!(block.get(&7).unwrap().pointer(), original);
            assert!(!block.is_dirty());
        }
        Ok::<_, ()>(())
    }).unwrap();
    assert_eq!(old.get(&7).unwrap().pointer(), original);
    drop(old);
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn actual_transaction_removal_exhausted_abort_restores_parent_and_outer_reader_custody() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(16 << 20);
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    storage
        .try_with_admitted_block(|block| {
            for order in 0..40 {
                assert!(put(block, &budget, order, 0x51).is_none());
            }
            Ok::<_, ()>(())
        })
        .unwrap();
    let old = storage.view();
    let original = (
        old.get(&7).unwrap().pointer(),
        old.get(&24).unwrap().pointer(),
    );
    let query7 = removal_key(&budget, 7);
    let query45 = removal_key(&budget, 45);
    let baseline = budget.reserved_bytes();
    let result = storage.try_with_admitted_block(|block| {
        drop(put(block, &budget, 7, 0x52));
        assert!(put(block, &budget, 45, 0x55).is_none());
        let parent = (
            block.get(&7).unwrap().pointer(),
            block.get(&45).unwrap().pointer(),
            block.get_before_block(&query7).unwrap().pointer(),
        );
        let held = budget.reserved_bytes();
        let records = NEXT_RECORD.load(SeqCst);
        let mut transaction = without_allocations(|| block.try_transaction_admitted()).unwrap();
        for order in (0..40).rev() {
            assert!(transaction_remove(&mut transaction, &budget, order).is_some());
        }
        assert!(transaction_remove(&mut transaction, &budget, 45).is_some());
        assert!(transaction_remove(&mut transaction, &budget, 99).is_none());
        assert!(transaction.is_empty());
        assert_eq!(transaction.touched_entries().len(), 42);
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
            .unwrap();
        assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
        without_allocations(|| drop(transaction));
        assert_eq!(budget.reserved_bytes(), held + blocker.remaining_bytes());
        reclaimed_since(records);
        assert_eq!(
            (
                block.get(&7).unwrap().pointer(),
                block.get(&45).unwrap().pointer(),
                block.get_before_block(&query7).unwrap().pointer()
            ),
            parent
        );
        assert_eq!(block.len(), 41);
        assert!(block.get_before_block(&query45).is_none());
        drop(blocker);
        let mut applied = block.try_transaction_admitted().unwrap();
        assert!(transaction_remove(&mut applied, &budget, 7).is_some());
        assert!(transaction_remove(&mut applied, &budget, 45).is_some());
        assert_eq!(
            applied.get_before_block(&query7).unwrap().pointer(),
            parent.2
        );
        assert!(applied.get_before_block(&query45).is_none());
        without_allocations(|| applied.apply());
        assert!(block.get(&7).is_none() && block.get(&45).is_none());
        assert_eq!(block.get_before_block(&query7).unwrap().pointer(), parent.2);
        Err::<(), _>("abort original parent after admitted child removal")
    });
    assert!(matches!(
        result,
        Err(AdmittedBlockError::Callback(
            "abort original parent after admitted child removal"
        ))
    ));
    assert_eq!(budget.reserved_bytes(), baseline);
    assert_eq!(storage.view().len(), 40);
    assert_eq!(
        (
            storage.view().get(&7).unwrap().pointer(),
            storage.view().get(&24).unwrap().pointer()
        ),
        original
    );
    assert_eq!(
        (
            old.get(&7).unwrap().pointer(),
            old.get(&24).unwrap().pointer()
        ),
        original
    );
    drop((old, query7, query45));
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

fn caught_remove_failure(
    key: Payload,
    counters: &Counters,
    fault: u8,
    remove: impl FnOnce(Payload) -> Result<Option<Payload>, (Payload, AdmittedStorageError)>,
) {
    let query_id = key.id();
    let records = NEXT_RECORD.load(SeqCst);
    let copies = counters.keys.load(SeqCst);
    if fault == 0 {
        PANIC_CHARGE.store(query_id, SeqCst);
    } else {
        FACTORY_FAULT.with(|mode| mode.set(fault));
    }
    let removed = catch_unwind(AssertUnwindSafe(|| {
        let _ = remove(key);
    }));
    assert!(
        removed.is_err(),
        "actual removal callback/copy/query-drop must fail"
    );
    if fault == 3 {
        assert_eq!(NEXT_RECORD.load(SeqCst), records);
        assert_eq!(counters.keys.load(SeqCst), copies);
    } else if fault == 4 {
        assert!(NEXT_RECORD.load(SeqCst) > records);
        assert!(counters.keys.load(SeqCst) > copies);
    } else {
        assert_eq!(PANIC_CHARGE.load(SeqCst), usize::MAX);
        assert!(RECORDS[query_id].freed.load(SeqCst));
        assert!(RECORDS[query_id].refunded.load(SeqCst));
    }
}

#[test]
fn actual_removal_caught_copy_and_consumed_query_panics_cannot_publish() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for (in_transaction, fault, order, repeated) in [
        (false, 3, 7, false),
        (false, 4, 7, false),
        (false, 0, 7, false),
        (false, 0, 9, false),
        (true, 3, 7, false),
        (true, 4, 7, false),
        (true, 4, 7, true),
        (true, 0, 7, false),
        (true, 0, 9, false),
    ] {
        reset();
        let budget = AllocationBudget::new(8 << 20);
        let counters = Arc::new(Counters::default());
        let _context = PolicyContext::new(&counters);
        let mut storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
        seed(&storage, &budget, 0x61);
        storage
            .try_with_admitted_block(|block| {
                drop(put(block, &budget, 7, 0x62));
                assert!(put(block, &budget, 8, 0x68).is_none());
                Ok::<_, ()>(())
            })
            .unwrap();
        let before = {
            let history = storage.history();
            (
                history.current().get(&7).unwrap().pointer(),
                history.current().get(&8).unwrap().pointer(),
                history.get_before_block(&7).unwrap().pointer(),
            )
        };
        let old = storage.view();
        let held = budget.reserved_bytes();
        let records = NEXT_RECORD.load(SeqCst);
        let callback_returned_ok = Cell::new(false);
        let aggregate = catch_unwind(AssertUnwindSafe(|| {
            storage.try_with_admitted_block(|block| {
                if in_transaction {
                    let mut transaction = block.try_transaction_admitted().unwrap();
                    if repeated {
                        drop(transaction_put(&mut transaction, &budget, 7, 0x63));
                    }
                    let key = removal_key(&budget, order);
                    caught_remove_failure(key, &counters, fault, |key| {
                        transaction.try_remove_admitted(key)
                    });
                    assert!(catch_unwind(AssertUnwindSafe(|| transaction.apply())).is_err());
                } else {
                    let key = removal_key(&budget, order);
                    caught_remove_failure(key, &counters, fault, |key| {
                        block.try_remove_admitted(key)
                    });
                }
                callback_returned_ok.set(true);
                Ok::<_, ()>(())
            })
        }));
        assert!(callback_returned_ok.get());
        assert!(
            aggregate.is_err(),
            "caught removal failure cannot publish either original map"
        );
        assert_eq!(budget.reserved_bytes(), held);
        reclaimed_since(records);
        assert_eq!(
            (
                old.get(&7).unwrap().pointer(),
                old.get(&8).unwrap().pointer()
            ),
            (before.0, before.1)
        );
        drop(old);
        {
            let history = storage.history();
            assert_eq!(
                (
                    history.current().get(&7).unwrap().pointer(),
                    history.current().get(&8).unwrap().pointer(),
                    history.get_before_block(&7).unwrap().pointer()
                ),
                before
            );
            assert!(history.get_before_block(&8).is_none());
        }
        assert!(matches!(
            storage.try_with_admitted_block(|_| -> Result<(), ()> {
                panic!("poisoned removal owner cannot invoke callback")
            }),
            Err(AdmittedBlockError::Admission(
                AdmittedStorageError::Poisoned { .. }
            ))
        ));
        without_allocations(|| drop(storage));
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
