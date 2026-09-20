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
