//! Closed map edits with real MV credits and observed allocation custody.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    borrow::Borrow,
    cell::Cell,
    cmp::Ordering,
    future::Future,
    mem::ManuallyDrop,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
    task::{Context, Wake, Waker},
};

use concread::bptree::{
    AllocationDemand, BptreeMap, BptreeMapCheckpoint, BptreeMapOwned, ClonePlanning,
    MapAdmissionError, NodeCloning, NodeFunding, OwnedWriteError, PlanningError, Prepaid,
};
use mv::allocation::{
    AllocationBudget, AllocationCharge, AllocationRefusal, AllocationReservation,
};

static SERIAL: Mutex<()> = Mutex::new(());
static RECORDS: [Record; 16_384] = [const { Record::new() }; 16_384];
static NEXT_RECORD: AtomicUsize = AtomicUsize::new(0);
static PANIC_CHARGE: AtomicUsize = AtomicUsize::new(usize::MAX);

struct Record {
    pointer: AtomicUsize,
    bytes: AtomicUsize,
    alignment: AtomicUsize,
    reclaiming: AtomicBool,
    freed: AtomicBool,
    refunded: AtomicBool,
}

impl Record {
    const fn new() -> Self {
        Self {
            pointer: AtomicUsize::new(0),
            bytes: AtomicUsize::new(0),
            alignment: AtomicUsize::new(0),
            reclaiming: AtomicBool::new(false),
            freed: AtomicBool::new(false),
            refunded: AtomicBool::new(false),
        }
    }
}

#[derive(Clone, Copy)]
struct Expected {
    id: usize,
    layout: Layout,
}

thread_local! {
    // Cursor and next-reader charges are split together before either shell is
    // allocated. The observer records their actual requested layouts separately.
    static EXPECTED: Cell<[Option<Expected>; 2]> = const { Cell::new([None; 2]) };
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

struct ObservedAllocator;

unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forward the caller's original layout to the actual allocator.
        let pointer = unsafe { System.alloc(layout) };
        let _ = ALLOCATIONS.try_with(|count| {
            if let Some(value) = count.get() {
                count.set(Some(value + 1));
            }
        });
        if !pointer.is_null() {
            let _ = EXPECTED.try_with(|pending| {
                let mut slots = pending.get();
                if let Some(index) = slots
                    .iter()
                    .position(|slot| slot.is_some_and(|entry| entry.layout == layout))
                {
                    let expected = slots[index].take().unwrap();
                    let record = &RECORDS[expected.id];
                    record.bytes.store(layout.size(), SeqCst);
                    record.alignment.store(layout.align(), SeqCst);
                    record.pointer.store(pointer as usize, SeqCst);
                    pending.set(slots);
                }
            });
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // Claim the live allocation before System can recycle its address.
        // A concurrent replacement at that address has a different record and
        // must not inherit this free witness. The witness itself remains false
        // until the original physical deallocation has returned.
        let original = RECORDS[..NEXT_RECORD.load(SeqCst)].iter().find(|record| {
            record.pointer.load(SeqCst) == pointer as usize
                && record.bytes.load(SeqCst) == layout.size()
                && record.alignment.load(SeqCst) == layout.align()
                && !record.freed.load(SeqCst)
                && record
                    .reclaiming
                    .compare_exchange(false, true, SeqCst, SeqCst)
                    .is_ok()
        });
        // SAFETY: this is the original allocation pointer and its supplied layout.
        unsafe { System.dealloc(pointer, layout) };
        if let Some(record) = original {
            record.freed.store(true, SeqCst);
        }
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

fn reset() {
    PANIC_CHARGE.store(usize::MAX, SeqCst);
    assert!(EXPECTED.with(|pending| pending.get().iter().all(Option::is_none)));
    for record in &RECORDS[..NEXT_RECORD.swap(0, SeqCst)] {
        record.pointer.store(0, SeqCst);
        record.bytes.store(0, SeqCst);
        record.alignment.store(0, SeqCst);
        record.reclaiming.store(false, SeqCst);
        record.freed.store(false, SeqCst);
        record.refunded.store(false, SeqCst);
    }
}

fn counted<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    struct Window;
    impl Drop for Window {
        fn drop(&mut self) {
            ALLOCATIONS.with(|count| count.set(None));
        }
    }
    ALLOCATIONS.with(|count| assert!(count.replace(Some(0)).is_none()));
    let window = Window;
    let result = operation();
    let allocations = ALLOCATIONS.with(|count| count.get().unwrap());
    drop(window);
    (result, allocations)
}

fn without_allocations<T>(operation: impl FnOnce() -> T) -> T {
    let (result, allocations) = counted(operation);
    assert_eq!(
        allocations, 0,
        "operation allocated after its admission boundary"
    );
    result
}

#[derive(Debug)]
struct Charge {
    id: usize,
    credit: AllocationCharge,
}

impl Charge {
    fn split(reservation: &mut AllocationReservation, layout: Layout) -> Self {
        let credit = reservation
            .try_split(layout)
            .expect("complete original demand");
        if layout.size() == 0 {
            // An empty fixed buffer owns a real zero-layout charge but makes
            // no System allocation. Do not fabricate an allocator witness.
            return Self {
                id: usize::MAX,
                credit,
            };
        }
        let id = NEXT_RECORD.fetch_add(1, SeqCst);
        assert!(id < RECORDS.len());
        EXPECTED.with(|pending| {
            let mut slots = pending.get();
            let slot = slots.iter_mut().find(|slot| slot.is_none()).unwrap();
            *slot = Some(Expected { id, layout });
            pending.set(slots);
        });
        Self { id, credit }
    }
}

impl Drop for Charge {
    fn drop(&mut self) {
        if self.credit.layout().size() == 0 {
            assert_eq!(self.id, usize::MAX);
            return;
        }
        let record = &RECORDS[self.id];
        assert_ne!(
            record.pointer.load(SeqCst),
            0,
            "charged storage was never observed"
        );
        assert!(
            record.freed.load(SeqCst),
            "credits returned before actual System free"
        );
        assert_eq!(record.bytes.load(SeqCst), self.credit.layout().size());
        assert_eq!(record.alignment.load(SeqCst), self.credit.layout().align());
        assert!(
            !record.refunded.swap(true, SeqCst),
            "same charge refunded twice"
        );
        // The real original AllocationCharge is dropped after these witnesses.
        assert!(
            PANIC_CHARGE
                .compare_exchange(self.id, usize::MAX, SeqCst, SeqCst)
                .is_err(),
            "injected original charge destructor panic",
        );
    }
}

#[derive(Debug)]
struct Payload {
    order: usize,
    bytes: ManuallyDrop<Box<[u8]>>,
    charge: ManuallyDrop<Charge>,
}

impl Payload {
    fn allocate(order: usize, length: usize, charge: Charge) -> Self {
        let mut bytes = Box::<[u8]>::new_uninit_slice(length);
        for byte in &mut bytes {
            byte.write(order as u8);
        }
        // SAFETY: every byte of this original exact-length allocation is initialized.
        let bytes = unsafe { bytes.assume_init() };
        Self {
            order,
            bytes: ManuallyDrop::new(bytes),
            charge: ManuallyDrop::new(charge),
        }
    }

    fn layout(&self) -> Layout {
        Layout::array::<u8>(self.bytes.len()).unwrap()
    }

    fn pointer(&self) -> usize {
        self.bytes.as_ptr() as usize
    }

    fn id(&self) -> usize {
        self.charge.id
    }
}

impl Drop for Payload {
    fn drop(&mut self) {
        // SAFETY: both fields are unique and destroyed once, actual Box first.
        unsafe {
            ManuallyDrop::drop(&mut self.bytes);
            ManuallyDrop::drop(&mut self.charge);
        }
    }
}

impl Clone for Payload {
    fn clone(&self) -> Self {
        panic!("ordinary Clone bypassed the original admitted payload policy")
    }
}

impl Borrow<usize> for Payload {
    fn borrow(&self) -> &usize {
        &self.order
    }
}

impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        self.order == other.order
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
        self.order.cmp(&other.order)
    }
}

#[derive(Default)]
struct Counters {
    admissions: AtomicUsize,
    keys: AtomicUsize,
    values: AtomicUsize,
}

struct Policy {
    reservation: AllocationReservation,
    counters: Arc<Counters>,
    copies: usize,
    fail_at: Option<usize>,
}

impl Policy {
    fn admit(
        budget: &AllocationBudget,
        counters: &Arc<Counters>,
        demand: AllocationDemand,
        fail_at: Option<usize>,
    ) -> Result<Self, AllocationRefusal> {
        counters.admissions.fetch_add(1, SeqCst);
        Ok(Self {
            reservation: budget.try_reserve_bytes(demand.bytes())?,
            counters: Arc::clone(counters),
            copies: 0,
            fail_at,
        })
    }

    fn copy(&mut self, source: &Payload) -> Payload {
        let charge = self.take_node_charge(source.layout());
        let mut result = Payload::allocate(source.order, source.bytes.len(), charge);
        result.bytes.copy_from_slice(&source.bytes);
        self.copies += 1;
        assert_ne!(
            self.fail_at,
            Some(self.copies),
            "injected nested clone refusal"
        );
        result
    }
}

impl NodeFunding for Policy {
    type Charge = Charge;

    fn take_node_charge(&mut self, layout: Layout) -> Charge {
        Charge::split(&mut self.reservation, layout)
    }
}

impl NodeCloning<Payload, Payload> for Policy {
    fn clone_key(&mut self, key: &Payload) -> Payload {
        self.counters.keys.fetch_add(1, SeqCst);
        self.copy(key)
    }

    fn clone_value(&mut self, value: &Payload) -> Payload {
        self.counters.values.fetch_add(1, SeqCst);
        self.copy(value)
    }
}

impl ClonePlanning<Payload, Payload> for Policy {
    fn plan_key(key: &Payload, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        demand.add_layout(key.layout())
    }

    fn plan_value(value: &Payload, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        demand.add_layout(value.layout())
    }
}

type Map = BptreeMap<Payload, Payload, Prepaid<Policy>>;
type Owned = BptreeMapOwned<Payload, Payload, Prepaid<Policy>>;

fn input(budget: &AllocationBudget, order: usize) -> (Payload, Payload) {
    // The large, irregular keys also occur at child minima. Their cloning demand
    // cannot be approximated from the selected leaf or an average payload size.
    let key_len = if order.is_multiple_of(7) {
        2049
    } else {
        17 + order % 31
    };
    let value_len = 49 + (order * 37) % 503;
    let key_layout = Layout::array::<u8>(key_len).unwrap();
    let value_layout = Layout::array::<u8>(value_len).unwrap();
    let mut reservation = budget
        .try_reserve_layouts([key_layout, value_layout])
        .unwrap();
    let key = Payload::allocate(order, key_len, Charge::split(&mut reservation, key_layout));
    let value = Payload::allocate(
        order,
        value_len,
        Charge::split(&mut reservation, value_layout),
    );
    (key, value)
}

fn map(budget: &AllocationBudget, counters: &Arc<Counters>) -> Map {
    budget.with_deferred_refund_notifications(|_| {
        Map::try_new_with_node_custody(|demand| Policy::admit(budget, counters, demand, None))
            .unwrap()
    })
}

fn insert(map: &Map, budget: &AllocationBudget, counters: &Arc<Counters>, order: usize) -> Owned {
    let (key, value) = input(budget, order);
    let start = NEXT_RECORD.load(SeqCst);
    let mut planned_allocations = 0;
    let ((owner, previous), allocations) = counted(|| {
        budget.with_deferred_refund_notifications(|_| {
            map.try_insert_admitted(key, value, |demand| {
                planned_allocations = demand.allocations();
                Policy::admit(budget, counters, demand, None)
            })
            .unwrap_or_else(|(_, error)| panic!("admission failed: {error:?}"))
        })
    });
    assert!(previous.is_none());
    assert_eq!(
        allocations,
        NEXT_RECORD.load(SeqCst) - start,
        "unowned insertion allocation"
    );
    assert!(allocations <= planned_allocations);
    assert_live_credits(budget);
    owner
}

fn commit(map: &Map, budget: &AllocationBudget, owner: Owned) {
    without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| {
            let writer = map
                .try_write_owned(owner)
                .unwrap_or_else(|_| panic!("original owner refused"));
            writer.commit();
        });
    });
}

fn assert_live_credits(budget: &AllocationBudget) {
    assert!(EXPECTED.with(|pending| pending.get().iter().all(Option::is_none)));
    let bytes: usize = RECORDS[..NEXT_RECORD.load(SeqCst)]
        .iter()
        .filter(|record| !record.refunded.load(SeqCst))
        .map(|record| record.bytes.load(SeqCst))
        .sum();
    assert_eq!(
        budget.reserved_bytes(),
        bytes,
        "unused operation credit escaped the handoff"
    );
}

fn reclaimed_since(start: usize) {
    for record in &RECORDS[start..NEXT_RECORD.load(SeqCst)] {
        assert_ne!(record.pointer.load(SeqCst), 0);
        assert!(record.freed.load(SeqCst));
        assert!(record.refunded.load(SeqCst));
    }
}

#[test]
fn complete_demand_refusal_allocates_nothing_and_retries_the_original_input_after_release() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let (key, value) = input(&budget, 7);
    let pointers = (key.pointer(), value.pointer());
    let blocking_credit = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
        .unwrap();
    let mut demand_bytes = 0;
    let ((key, value), error) = without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| {
            map.try_insert_admitted(key, value, |demand| {
                demand_bytes = demand.bytes();
                Policy::admit(&budget, &counters, demand, None)
            })
            .err()
            .expect("original capacity must refuse the complete operation")
        })
    });
    let MapAdmissionError::Refused(AllocationRefusal::Capacity {
        requested_bytes, ..
    }) = error
    else {
        panic!("refusal must originate in the original full budget");
    };
    assert_eq!(requested_bytes, demand_bytes);
    assert!(demand_bytes > 0);
    assert_eq!((key.pointer(), value.pointer()), pointers);
    assert!(map.read().is_empty());
    budget.with_deferred_refund_notifications(|_| drop(blocking_credit));
    let (owner, old) = budget.with_deferred_refund_notifications(|_| {
        map.try_insert_admitted(key, value, |demand| {
            Policy::admit(&budget, &counters, demand, None)
        })
        .unwrap_or_else(|_| panic!("released original capacity must admit retry"))
    });
    assert!(old.is_none());
    assert_eq!(owner.get(&7).unwrap().pointer(), pointers.1);
    assert_live_credits(&budget);
    commit(&map, &budget, owner);
    assert_eq!(map.read().get(&7).unwrap().pointer(), pointers.1);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn nonuniform_nested_payloads_split_and_grow_while_original_readers_retain_actual_credits() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(32 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    for order in 0..48 {
        let owner = insert(&map, &budget, &counters, order * 2);
        commit(&map, &budget, owner);
    }
    let original = map.read();
    let old_value = original.get(&0).unwrap();
    let old_id = old_value.id();
    let old_pointer = old_value.pointer();
    // Interleave both edges and interior splits. 128 entries cannot fit beneath
    // one eight-child branch in this engine, so this reaches a new root level.
    for order in 0..128 {
        if order < 96 && order % 2 == 0 {
            continue;
        }
        let owner = insert(&map, &budget, &counters, order);
        commit(&map, &budget, owner);
    }
    assert_eq!(map.read().len(), 128);
    assert_eq!(original.len(), 48);
    assert_eq!(old_value.pointer(), old_pointer);
    assert!(!RECORDS[old_id].freed.load(SeqCst));
    assert!(!RECORDS[old_id].refunded.load(SeqCst));
    assert_ne!(map.read().get(&0).unwrap().pointer(), old_pointer);
    assert!(
        counters.keys.load(SeqCst) > counters.values.load(SeqCst),
        "separators use explicit key cloning"
    );
    for (expected, (key, value)) in map.read().iter().enumerate() {
        assert_eq!(key.order, expected);
        assert_eq!(value.order, expected);
        assert!(value.bytes.iter().all(|byte| *byte == expected as u8));
    }
    let retained = budget.reserved_bytes();
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(original)));
    assert!(RECORDS[old_id].freed.load(SeqCst));
    assert!(RECORDS[old_id].refunded.load(SeqCst));
    assert!(budget.reserved_bytes() < retained);
    assert_live_credits(&budget);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn detached_public_owner_rejects_foreign_and_busy_maps_without_readmission_or_copy() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let counters = Arc::new(Counters::default());
    let original = map(&budget, &counters);
    let decoy = map(&budget, &counters);
    let owner = insert(&original, &budget, &counters, 1);
    let competing = insert(&original, &budget, &counters, 2);
    let pointer = owner.get(&1).unwrap().pointer();
    let admissions = counters.admissions.load(SeqCst);
    let records = NEXT_RECORD.load(SeqCst);
    let owner = without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| {
            let (owner, error) = decoy.try_write_owned(owner).err().expect("foreign owner");
            assert_eq!(error, OwnedWriteError::Changed);
            let held = original
                .try_write_owned(competing)
                .unwrap_or_else(|_| panic!("same base"));
            let (owner, error) = original.try_write_owned(owner).err().expect("held writer");
            assert_eq!(error, OwnedWriteError::Busy);
            assert_eq!(owner.get(&1).unwrap().pointer(), pointer);
            drop(held);
            let acquired = original
                .try_write_owned(owner)
                .unwrap_or_else(|_| panic!("released writer"));
            assert_eq!(acquired.get(&1).unwrap().pointer(), pointer);
            acquired.detach()
        })
    });
    assert_eq!(counters.admissions.load(SeqCst), admissions);
    assert_eq!(NEXT_RECORD.load(SeqCst), records);
    assert_eq!(owner.get(&1).unwrap().pointer(), pointer);
    commit(&original, &budget, owner);
    assert_eq!(original.read().get(&1).unwrap().pointer(), pointer);
    assert!(original.read().get(&2).is_none());
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop((decoy, original))));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn replacement_and_detached_successor_keep_their_original_storage_after_map_drop() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let first = insert(&map, &budget, &counters, 7);
    commit(&map, &budget, first);
    let original_id = map.read().get(&7).unwrap().id();
    let (key, value) = input(&budget, 7);
    let replacement_pointer = value.pointer();
    let (owner, previous) = budget.with_deferred_refund_notifications(|_| {
        map.try_insert_admitted(key, value, |demand| {
            Policy::admit(&budget, &counters, demand, None)
        })
        .unwrap_or_else(|_| panic!("replacement demand must fit"))
    });
    let previous = previous.expect("the original entry was replaced in the private cursor");
    let previous_id = previous.id();
    assert_ne!(
        previous_id, original_id,
        "published storage was copied by the explicit policy"
    );
    assert_eq!(owner.to_snapshot().len(), 1);
    assert_eq!(owner.get(&7).unwrap().pointer(), replacement_pointer);
    assert_live_credits(&budget);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    // The detached cursor retains its real original root and base generation.
    // The returned value independently retains its actual nested allocation.
    assert!(!RECORDS[original_id].freed.load(SeqCst));
    assert!(!RECORDS[previous_id].freed.load(SeqCst));
    assert_eq!(owner.get(&7).unwrap().pointer(), replacement_pointer);
    assert!(previous.bytes.iter().all(|byte| *byte == 7));
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(owner)));
    assert!(RECORDS[original_id].freed.load(SeqCst));
    assert!(!RECORDS[previous_id].freed.load(SeqCst));
    assert_live_credits(&budget);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(previous)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn every_partial_leaf_clone_unwind_reclaims_new_storage_and_preserves_published_references() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for fail_at in 1..=4 {
        reset();
        let budget = AllocationBudget::new(1 << 20);
        let counters = Arc::new(Counters::default());
        let map = map(&budget, &counters);
        for order in [0, 1] {
            let owner = insert(&map, &budget, &counters, order);
            commit(&map, &budget, owner);
        }
        let original = map.read();
        let old = original.get(&0).unwrap();
        let pointer = old.pointer();
        let baseline = budget.reserved_bytes();
        let start = NEXT_RECORD.load(SeqCst);
        let (key, value) = input(&budget, 9);
        let result = catch_unwind(AssertUnwindSafe(|| {
            budget.with_deferred_refund_notifications(|_| {
                let _ = map.try_insert_admitted(key, value, |demand| {
                    Policy::admit(&budget, &counters, demand, Some(fail_at))
                });
            });
        }));
        assert!(result.is_err());
        assert!(map.is_poisoned());
        assert_eq!(old.pointer(), pointer);
        assert_eq!(map.read().get(&0).unwrap().pointer(), pointer);
        assert_eq!(original.len(), 2);
        assert_eq!(budget.reserved_bytes(), baseline);
        reclaimed_since(start);
        let (key, value) = input(&budget, 10);
        let ((key, value), error) = without_allocations(|| {
            budget.with_deferred_refund_notifications(|_| {
                map.try_insert_admitted(key, value, |_| -> Result<Policy, ()> {
                    panic!("poisoned writer must not readmit")
                })
                .err()
                .expect("poison is explicit")
            })
        });
        assert!(matches!(error, MapAdmissionError::Poisoned));
        budget.with_deferred_refund_notifications(|_| drop((key, value, original)));
        without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

struct Reenter {
    map: Arc<Map>,
    budget: AllocationBudget,
    input: Mutex<Option<(Payload, Payload)>>,
    wakes: AtomicUsize,
    writer_released: AtomicBool,
}

impl Wake for Reenter {
    fn wake(self: Arc<Self>) {
        let mut slot = self.input.lock().unwrap();
        let (key, value) = slot.take().unwrap();
        let (input, error) = self.budget.with_deferred_refund_notifications(|_| {
            self.map
                .try_insert_admitted(key, value, |_| Err::<Policy, _>(()))
                .err()
                .expect("read-only reentrant probe refuses admission")
        });
        self.writer_released
            .store(matches!(error, MapAdmissionError::Refused(())), SeqCst);
        *slot = Some(input);
        self.wakes.fetch_add(1, SeqCst);
    }
}

#[test]
fn old_reader_and_abort_refunds_wake_only_after_the_original_writer_unlocks() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let counters = Arc::new(Counters::default());
    let map = Arc::new(map(&budget, &counters));
    let owner = insert(&map, &budget, &counters, 1);
    commit(&map, &budget, owner);
    let old = map.read();
    let old_id = old.get(&1).unwrap().id();
    let owner = insert(&map, &budget, &counters, 2);
    commit(&map, &budget, owner);
    let unpublished = insert(&map, &budget, &counters, 3);
    let wake = Arc::new(Reenter {
        map: Arc::clone(&map),
        budget: budget.clone(),
        input: Mutex::new(Some(input(&budget, 99))),
        wakes: AtomicUsize::new(0),
        writer_released: AtomicBool::new(false),
    });
    drop(wake.input.lock().unwrap());
    let waker = Waker::from(Arc::clone(&wake));
    let mut context = Context::from_waker(&waker);
    let mut wait = None;
    budget.with_deferred_refund_notifications(|_| {
        let held = map
            .try_write_owned(unpublished)
            .unwrap_or_else(|_| panic!("original writer"));
        let blocking_credit = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
            .unwrap();
        let AllocationRefusal::Capacity { release, .. } = budget.try_reserve_bytes(1).unwrap_err()
        else {
            panic!("original budget must be full");
        };
        let mut pending = Box::pin(release.wait_for_release());
        assert!(pending.as_mut().poll(&mut context).is_pending());
        wait = Some(pending);
        without_allocations(|| {
            drop(old);
            assert!(RECORDS[old_id].freed.load(SeqCst));
            assert!(RECORDS[old_id].refunded.load(SeqCst));
            assert!(budget.reserved_bytes() < budget.limit_bytes());
            assert_eq!(wake.wakes.load(SeqCst), 0);
            drop(held);
            drop(blocking_credit);
            assert_eq!(wake.wakes.load(SeqCst), 0);
        });
    });
    assert_eq!(wake.wakes.load(SeqCst), 1);
    assert!(wake.writer_released.load(SeqCst));
    assert!(
        wait.as_mut()
            .unwrap()
            .as_mut()
            .poll(&mut context)
            .is_ready()
    );
    assert_eq!(map.read().len(), 2);
    assert!(map.read().get(&3).is_none());
    budget.with_deferred_refund_notifications(|_| drop((wait, waker, wake)));
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

fn edit(
    map: &Map,
    budget: &AllocationBudget,
    counters: &Arc<Counters>,
    owner: Owned,
    key: Payload,
    value: Payload,
) -> (Owned, Option<Payload>) {
    let start = NEXT_RECORD.load(SeqCst);
    let mut planned_allocations = 0;
    let (result, allocations) = counted(|| {
        budget.with_deferred_refund_notifications(|_| {
            map.try_insert_owned_admitted(owner, key, value, |demand| {
                planned_allocations = demand.allocations();
                Policy::admit(budget, counters, demand, None)
            })
            .unwrap_or_else(|(_, error)| panic!("retained edit refused: {error:?}"))
        })
    });
    assert_eq!(
        allocations,
        NEXT_RECORD.load(SeqCst) - start,
        "unowned retained-edit allocation"
    );
    assert!(allocations <= planned_allocations);
    assert_live_credits(budget);
    result
}

#[test]
fn retained_successor_grows_and_replaces_entries_before_one_atomic_publication() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(32 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let original = map.read();
    let mut owner = insert(&map, &budget, &counters, 0);
    let mut pointers = [0; 128];
    pointers[0] = owner.get(&0).unwrap().pointer();
    for index in 1..128 {
        let order = index * 73 % 128;
        let (key, value) = input(&budget, order);
        pointers[order] = value.pointer();
        let (next, previous) = edit(&map, &budget, &counters, owner, key, value);
        assert!(previous.is_none());
        owner = next;
        assert_eq!(owner.to_snapshot().len(), index + 1);
        assert!(
            map.read().is_empty(),
            "an intermediate private edit became visible"
        );
    }
    for (order, pointer) in pointers.into_iter().enumerate() {
        assert_eq!(
            owner.get(&order).unwrap().pointer(),
            pointer,
            "private entry copied again"
        );
    }
    let (key, value) = input(&budget, 7);
    let replacement = value.pointer();
    let (owner, previous) = edit(&map, &budget, &counters, owner, key, value);
    let previous = previous.expect("same private entry replacement");
    assert_eq!(previous.pointer(), pointers[7]);
    assert_eq!(owner.to_snapshot().len(), 128);
    budget.with_deferred_refund_notifications(|_| drop(previous));
    commit(&map, &budget, owner);
    assert!(original.is_empty());
    let published = map.read();
    assert_eq!(published.len(), 128);
    assert_eq!(published.get(&7).unwrap().pointer(), replacement);
    without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| drop((published, original)))
    });
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn retained_capacity_refusal_preserves_private_entries_and_input_then_retries() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let owner = insert(&map, &budget, &counters, 0);
    let private_pointer = owner.get(&0).unwrap().pointer();
    let (key, value) = input(&budget, 7);
    let pointers = (key.pointer(), value.pointer());
    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
        .unwrap();
    let records = NEXT_RECORD.load(SeqCst);
    let ((owner, (key, value)), error) = without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| {
            map.try_insert_owned_admitted(owner, key, value, |demand| {
                Policy::admit(&budget, &counters, demand, None)
            })
            .err()
            .expect("full capacity must refuse retained edit")
        })
    });
    assert!(matches!(
        error,
        MapAdmissionError::Refused(AllocationRefusal::Capacity { .. })
    ));
    assert_eq!(NEXT_RECORD.load(SeqCst), records);
    assert_eq!((key.pointer(), value.pointer()), pointers);
    assert_eq!(owner.get(&0).unwrap().pointer(), private_pointer);
    assert_eq!(owner.to_snapshot().len(), 1);
    assert!(map.read().is_empty());
    assert!(!map.is_poisoned());
    budget.with_deferred_refund_notifications(|_| drop(blocker));
    let (owner, previous) = edit(&map, &budget, &counters, owner, key, value);
    assert!(previous.is_none());
    assert_eq!(owner.get(&7).unwrap().pointer(), pointers.1);
    commit(&map, &budget, owner);
    assert_eq!(map.read().len(), 2);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
}

#[test]
fn retained_edits_refuse_foreign_busy_and_changed_generations_before_admission() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(2 << 20);
    let counters = Arc::new(Counters::default());
    let original = map(&budget, &counters);
    let foreign = map(&budget, &counters);
    let owner = insert(&original, &budget, &counters, 0);
    let competing = insert(&original, &budget, &counters, 1);
    let (key, value) = input(&budget, 7);
    let pointers = (
        key.pointer(),
        value.pointer(),
        owner.get(&0).unwrap().pointer(),
    );
    let never = |_| -> Result<Policy, ()> { panic!("invalid owner was readmitted") };
    let (owner, key, value) = without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| {
            let ((owner, (key, value)), error) = foreign
                .try_insert_owned_admitted(owner, key, value, never)
                .err()
                .unwrap();
            assert!(matches!(error, MapAdmissionError::Changed));
            let held = original
                .try_write_owned(competing)
                .unwrap_or_else(|_| panic!("competing writer"));
            let ((owner, (key, value)), error) = original
                .try_insert_owned_admitted(owner, key, value, never)
                .err()
                .unwrap();
            assert!(matches!(error, MapAdmissionError::Busy));
            held.commit();
            let ((owner, (key, value)), error) = original
                .try_insert_owned_admitted(owner, key, value, never)
                .err()
                .unwrap();
            assert!(matches!(error, MapAdmissionError::Changed));
            assert_eq!(
                (
                    key.pointer(),
                    value.pointer(),
                    owner.get(&0).unwrap().pointer()
                ),
                pointers
            );
            (owner, key, value)
        })
    });
    assert_eq!(original.read().len(), 1);
    assert!(original.read().get(&1).is_some());
    assert!(original.read().get(&0).is_none());
    without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| drop((owner, key, value, original, foreign)))
    });
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn fully_exhausted_budget_can_abort_all_retained_edits_without_allocating() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let original = insert(&map, &budget, &counters, 255);
    commit(&map, &budget, original);
    let baseline = budget.reserved_bytes();
    let original_pointer = map.read().get(&255).unwrap().pointer();
    let start = NEXT_RECORD.load(SeqCst);
    let mut owner = insert(&map, &budget, &counters, 0);
    for order in 1..64 {
        let (key, value) = input(&budget, order);
        let (next, previous) = edit(&map, &budget, &counters, owner, key, value);
        assert!(previous.is_none());
        owner = next;
    }
    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
        .unwrap();
    let blocked_bytes = blocker.remaining_bytes();
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(owner)));
    assert_eq!(budget.reserved_bytes(), baseline + blocked_bytes);
    assert_eq!(map.read().get(&255).unwrap().pointer(), original_pointer);
    assert_eq!(map.read().len(), 1);
    reclaimed_since(start);
    budget.with_deferred_refund_notifications(|_| drop(blocker));
    assert_live_credits(&budget);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
}

#[test]
fn later_copy_unwind_aborts_the_whole_private_successor_and_preserves_published_storage() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for fail_at in 1..=4 {
        reset();
        let budget = AllocationBudget::new(8 << 20);
        let counters = Arc::new(Counters::default());
        let map = map(&budget, &counters);
        for order in 0..48 {
            let owner = insert(&map, &budget, &counters, order);
            commit(&map, &budget, owner);
        }
        let old = map.read();
        let original_pointer = old.get(&0).unwrap().pointer();
        let baseline = budget.reserved_bytes();
        let start = NEXT_RECORD.load(SeqCst);
        let owner = insert(&map, &budget, &counters, 100);
        let (key, value) = input(&budget, 0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            budget.with_deferred_refund_notifications(|_| {
                let _ = map.try_insert_owned_admitted(owner, key, value, |demand| {
                    Policy::admit(&budget, &counters, demand, Some(fail_at))
                });
            });
        }));
        assert!(
            result.is_err(),
            "the untouched published leaf must be copied"
        );
        assert!(map.is_poisoned());
        assert_eq!(map.read().get(&0).unwrap().pointer(), original_pointer);
        assert_eq!(map.read().len(), 48);
        assert!(map.read().get(&100).is_none());
        assert_eq!(budget.reserved_bytes(), baseline);
        reclaimed_since(start);
        without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(old)));
        without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn private_leaf_split_unwind_reclaims_all_previous_edits_without_publication() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let probe = map(&budget, &counters);
    let mut owner = insert(&probe, &budget, &counters, 0);
    let mut split = None;
    for order in 1..16 {
        let before = counters.keys.load(SeqCst);
        let (key, value) = input(&budget, order);
        let (next, previous) = edit(&probe, &budget, &counters, owner, key, value);
        assert!(previous.is_none());
        owner = next;
        let copies = counters.keys.load(SeqCst) - before;
        if copies > 0 {
            split = Some((order, copies));
            break;
        }
    }
    let (split_at, copies) = split.expect("first private leaf split within both node geometries");
    budget.with_deferred_refund_notifications(|_| drop((owner, probe)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
    for fail_at in 1..=copies {
        reset();
        let map = map(&budget, &counters);
        let baseline = budget.reserved_bytes();
        let start = NEXT_RECORD.load(SeqCst);
        let mut owner = insert(&map, &budget, &counters, 0);
        for order in 1..split_at {
            let (key, value) = input(&budget, order);
            let (next, previous) = edit(&map, &budget, &counters, owner, key, value);
            assert!(previous.is_none());
            owner = next;
        }
        let (key, value) = input(&budget, split_at);
        let result = catch_unwind(AssertUnwindSafe(|| {
            budget.with_deferred_refund_notifications(|_| {
                let _ = map.try_insert_owned_admitted(owner, key, value, |demand| {
                    Policy::admit(&budget, &counters, demand, Some(fail_at))
                });
            });
        }));
        assert!(result.is_err());
        assert!(map.is_poisoned());
        assert!(map.read().is_empty());
        assert_eq!(budget.reserved_bytes(), baseline);
        reclaimed_since(start);
        without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn retired_tracking_charge_unwind_sees_installed_bookkeeping_and_aborts_all_private_nodes() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let baseline = budget.reserved_bytes();
    let start = NEXT_RECORD.load(SeqCst);
    let owner = insert(&map, &budget, &counters, 0);
    // Original insertion records its two incoming payloads first, then the
    // original first_seen tracking allocation before any shell or node copy.
    let first_tracking = start + 2;
    assert!(!RECORDS[first_tracking].freed.load(SeqCst));
    let (key, value) = input(&budget, 1);
    PANIC_CHARGE.store(first_tracking, SeqCst);
    let result = catch_unwind(AssertUnwindSafe(|| {
        budget.with_deferred_refund_notifications(|_| {
            let _ = map.try_insert_owned_admitted(owner, key, value, |demand| {
                Policy::admit(&budget, &counters, demand, None)
            });
        });
    }));
    assert!(result.is_err());
    assert_eq!(
        PANIC_CHARGE.load(SeqCst),
        usize::MAX,
        "the old tracking charge must unwind"
    );
    assert!(map.is_poisoned());
    assert!(map.read().is_empty());
    assert_eq!(budget.reserved_bytes(), baseline);
    reclaimed_since(start);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

fn checkpoint_insert(
    checkpoint: &mut BptreeMapCheckpoint<'_, Payload, Payload, Prepaid<Policy>>,
    budget: &AllocationBudget,
    counters: &Arc<Counters>,
    order: usize,
) -> Option<Payload> {
    let (key, value) = input(budget, order);
    let start = NEXT_RECORD.load(SeqCst);
    let mut bound = 0;
    let (result, allocations) = counted(|| {
        checkpoint
            .try_insert_admitted(key, value, |demand| {
                bound = demand.allocations();
                Policy::admit(budget, counters, demand, None)
            })
            .unwrap_or_else(|(_, error)| panic!("checkpoint demand refused: {error:?}"))
    });
    assert!(allocations <= bound);
    assert_eq!(allocations, NEXT_RECORD.load(SeqCst) - start);
    assert_live_credits(budget);
    result
}

#[test]
fn full_budget_checkpoint_abort_restores_original_private_entries_buffers_and_credits() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let first_record = NEXT_RECORD.load(SeqCst);
    let owner = insert(&map, &budget, &counters, 0);
    let parent_pointer = owner.get(&0).unwrap().pointer();
    let parent_first = first_record + 2;
    let parent_last = first_record + 3;
    let baseline = budget.reserved_bytes();
    let start = NEXT_RECORD.load(SeqCst);
    budget.with_deferred_refund_notifications(|_| {
        let mut writer = map
            .try_write_owned(owner)
            .unwrap_or_else(|_| panic!("original writer"));
        let mut child = without_allocations(|| writer.checkpoint().unwrap());
        for order in 1..80 {
            assert!(checkpoint_insert(&mut child, &budget, &counters, order).is_none());
        }
        assert_eq!(child.to_snapshot().len(), 80);
        assert!(!RECORDS[parent_first].freed.load(SeqCst));
        assert!(!RECORDS[parent_last].freed.load(SeqCst));
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
            .unwrap();
        let blocked_bytes = blocker.remaining_bytes();
        without_allocations(|| drop(child));
        assert_eq!(budget.reserved_bytes(), baseline + blocked_bytes);
        assert_eq!(writer.get(&0).unwrap().pointer(), parent_pointer);
        assert_eq!(writer.len(), 1);
        assert!(!RECORDS[parent_first].freed.load(SeqCst));
        assert!(!RECORDS[parent_last].freed.load(SeqCst));
        reclaimed_since(start);
        drop(blocker);
        without_allocations(|| writer.commit());
    });
    assert_eq!(map.read().len(), 1);
    assert_eq!(map.read().get(&0).unwrap().pointer(), parent_pointer);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn nested_checkpoint_apply_abort_and_sibling_apply_preserve_original_parent_until_commit() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(16 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let old = map.read();
    let owner = insert(&map, &budget, &counters, 0);
    let parent_pointer = owner.get(&0).unwrap().pointer();
    let baseline = budget.reserved_bytes();
    budget.with_deferred_refund_notifications(|_| {
        let mut writer = map
            .try_write_owned(owner)
            .unwrap_or_else(|_| panic!("original writer"));
        let mut outer = writer.checkpoint().unwrap();
        let mut nested = outer.checkpoint().unwrap();
        for order in 1..40 {
            assert!(checkpoint_insert(&mut nested, &budget, &counters, order).is_none());
        }
        without_allocations(|| nested.apply());
        assert_eq!(outer.to_snapshot().len(), 40);
        let outer_pointer = outer.get(&0).unwrap().pointer();
        let outer_baseline = budget.reserved_bytes();
        let mut aborted = outer.checkpoint().unwrap();
        let previous = checkpoint_insert(&mut aborted, &budget, &counters, 0).unwrap();
        drop(previous);
        assert!(checkpoint_insert(&mut aborted, &budget, &counters, 999).is_none());
        without_allocations(|| drop(aborted));
        assert_eq!(outer.get(&0).unwrap().pointer(), outer_pointer);
        assert!(outer.get(&999).is_none());
        assert_eq!(budget.reserved_bytes(), outer_baseline);
        let mut applied = outer.checkpoint().unwrap();
        for order in (40..80).rev() {
            assert!(checkpoint_insert(&mut applied, &budget, &counters, order).is_none());
        }
        without_allocations(|| applied.apply());
        assert_eq!(outer.to_snapshot().len(), 80);
        without_allocations(|| drop(outer));
        assert_eq!(writer.get(&0).unwrap().pointer(), parent_pointer);
        assert_eq!(writer.len(), 1);
        assert_eq!(budget.reserved_bytes(), baseline);
        let (key, value) = input(&budget, 6);
        assert!(
            writer
                .try_insert_admitted(key, value, |demand| {
                    Policy::admit(&budget, &counters, demand, None)
                })
                .unwrap_or_else(|_| panic!("admitted edit under the original writer"))
                .is_none()
        );
        let mut sibling = writer.checkpoint().unwrap();
        assert!(checkpoint_insert(&mut sibling, &budget, &counters, 7).is_none());
        without_allocations(|| sibling.apply());
        assert!(old.is_empty());
        assert!(map.read().is_empty());
        without_allocations(|| writer.commit());
    });
    assert!(old.is_empty());
    assert_eq!(map.read().len(), 3);
    assert!(map.read().get(&6).is_some());
    assert!(map.read().get(&7).is_some());
    assert!(map.read().get(&1).is_none());
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(old)));
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn caught_checkpoint_edit_panic_cannot_read_detach_or_publish_the_original_cursor() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for detach in [false, true] {
        reset();
        let budget = AllocationBudget::new(2 << 20);
        let counters = Arc::new(Counters::default());
        let map = map(&budget, &counters);
        let initial = insert(&map, &budget, &counters, 0);
        commit(&map, &budget, initial);
        let original_pointer = map.read().get(&0).unwrap().pointer();
        let baseline = budget.reserved_bytes();
        let start = NEXT_RECORD.load(SeqCst);
        let owner = insert(&map, &budget, &counters, 1);
        budget.with_deferred_refund_notifications(|_| {
            let mut writer = map
                .try_write_owned(owner)
                .unwrap_or_else(|_| panic!("original writer"));
            let mut child = writer.checkpoint().unwrap();
            let (key, value) = input(&budget, 7);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = child.try_insert_admitted(key, value, |demand| {
                        Policy::admit(&budget, &counters, demand, Some(1))
                    });
                }))
                .is_err()
            );
            assert!(catch_unwind(AssertUnwindSafe(|| child.get(&0))).is_err());
            assert!(catch_unwind(AssertUnwindSafe(|| child.to_snapshot().len())).is_err());
            assert!(catch_unwind(AssertUnwindSafe(|| child.apply())).is_err());
            assert!(catch_unwind(AssertUnwindSafe(|| writer.get(&0))).is_err());
            let refused = catch_unwind(AssertUnwindSafe(|| {
                if detach {
                    drop(writer.detach());
                } else {
                    writer.commit();
                }
            }));
            assert!(refused.is_err());
        });
        assert!(map.is_poisoned());
        assert_eq!(map.read().get(&0).unwrap().pointer(), original_pointer);
        assert_eq!(map.read().len(), 1);
        assert_eq!(budget.reserved_bytes(), baseline);
        reclaimed_since(start);
        without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
        reclaimed_since(0);
    }
}

#[test]
fn checkpoint_capacity_refusal_keeps_child_state_and_original_input_for_retry() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let owner = insert(&map, &budget, &counters, 0);
    budget.with_deferred_refund_notifications(|_| {
        let mut writer = map
            .try_write_owned(owner)
            .unwrap_or_else(|_| panic!("original writer"));
        let mut child = writer.checkpoint().unwrap();
        assert!(checkpoint_insert(&mut child, &budget, &counters, 1).is_none());
        let original = child.get(&1).unwrap().pointer();
        let (key, value) = input(&budget, 7);
        let pointers = (key.pointer(), value.pointer());
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
            .unwrap();
        let ((key, value), error) = without_allocations(|| {
            child
                .try_insert_admitted(key, value, |demand| {
                    Policy::admit(&budget, &counters, demand, None)
                })
                .expect_err("original full budget")
        });
        assert!(matches!(
            error,
            MapAdmissionError::Refused(AllocationRefusal::Capacity { .. })
        ));
        assert_eq!((key.pointer(), value.pointer()), pointers);
        assert_eq!(child.to_snapshot().len(), 2);
        assert_eq!(child.get(&1).unwrap().pointer(), original);
        drop(blocker);
        assert!(
            child
                .try_insert_admitted(key, value, |demand| {
                    Policy::admit(&budget, &counters, demand, None)
                })
                .unwrap_or_else(|_| panic!("same input retry"))
                .is_none()
        );
        assert_eq!(child.get(&7).unwrap().pointer(), pointers.1);
        without_allocations(|| child.apply());
        without_allocations(|| writer.commit());
    });
    assert_eq!(map.read().len(), 3);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn checkpoint_buffer_refund_panic_restores_parent_ownership_and_forbids_publication() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let owner = insert(&map, &budget, &counters, 0);
    let parent_id = owner.get(&0).unwrap().id();
    let baseline = budget.reserved_bytes();
    let start = NEXT_RECORD.load(SeqCst);
    budget.with_deferred_refund_notifications(|_| {
        let mut writer = map
            .try_write_owned(owner)
            .unwrap_or_else(|_| panic!("original writer"));
        let mut child = writer.checkpoint().unwrap();
        let (key, value) = input(&budget, 1);
        // Both buffers grow on this first child edit. Its first recorded charge
        // is the new first_seen buffer; the checkpoint retains the displaced one.
        let child_buffer = NEXT_RECORD.load(SeqCst);
        assert!(
            child
                .try_insert_admitted(key, value, |demand| {
                    Policy::admit(&budget, &counters, demand, None)
                })
                .unwrap_or_else(|_| panic!("child edit"))
                .is_none()
        );
        PANIC_CHARGE.store(child_buffer, SeqCst);
        assert!(catch_unwind(AssertUnwindSafe(|| drop(child))).is_err());
        assert_eq!(PANIC_CHARGE.load(SeqCst), usize::MAX);
        assert_eq!(budget.reserved_bytes(), baseline);
        assert!(!RECORDS[parent_id].freed.load(SeqCst));
        reclaimed_since(start);
        assert!(map.read().is_empty());
        assert!(catch_unwind(AssertUnwindSafe(|| writer.len())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| writer.commit())).is_err());
    });
    assert!(map.is_poisoned());
    assert!(map.read().is_empty());
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn admitted_removal_funds_all_path_sibling_and_separator_copies_until_empty() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for direction in 0..3 {
        reset();
        let budget = AllocationBudget::new(32 << 20);
        let counters = Arc::new(Counters::default());
        let map = map(&budget, &counters);
        for order in 0..128 {
            let owner = insert(&map, &budget, &counters, order);
            commit(&map, &budget, owner);
        }
        let original = map.read();
        let original_pointer = original.get(&0).unwrap().pointer();
        let original_id = original.get(&0).unwrap().id();
        let mut present = [true; 128];
        budget.with_deferred_refund_notifications(|_| {
            for step in 0..128 {
                // Ascending/descending force both edge sibling cases; this odd
                // permutation visits every interior key exactly once as well.
                let order = match direction {
                    0 => step,
                    1 => 127 - step,
                    _ => (step * 73) % 128,
                };
                let (key, unused) = input(&budget, order);
                drop(unused);
                let mut writer = map
                    .try_write_admitted(|demand| Policy::admit(&budget, &counters, demand, None))
                    .unwrap();
                let demand = without_allocations(|| writer.removal_demand(&key).unwrap());
                let start = NEXT_RECORD.load(SeqCst);
                let (previous, allocations) = counted(|| {
                    writer
                        .try_remove_admitted(&key, |actual| {
                            assert_eq!(actual, demand);
                            Policy::admit(&budget, &counters, actual, None)
                        })
                        .unwrap()
                        .unwrap()
                });
                assert_eq!(previous.order, order);
                assert_eq!(
                    allocations,
                    NEXT_RECORD.load(SeqCst) - start,
                    "removal allocated outside original credit custody"
                );
                assert!(allocations <= demand.allocations());
                drop(previous);
                drop(key);
                present[order] = false;
                assert_eq!(writer.len(), 127 - step);
                without_allocations(|| writer.commit());
                let current = map.read();
                for (index, expected) in present.iter().enumerate() {
                    assert_eq!(current.get(&index).is_some(), *expected);
                }
                assert_eq!(original.len(), 128);
                assert_eq!(original.get(&0).unwrap().pointer(), original_pointer);
                assert!(!RECORDS[original_id].freed.load(SeqCst));
                assert_live_credits(&budget);
            }
            assert!(map.read().is_empty());
            without_allocations(|| drop(original));
        });
        without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn admitted_removal_refusal_and_absence_preserve_original_private_generation() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(4 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    for order in 0..32 {
        let owner = insert(&map, &budget, &counters, order);
        commit(&map, &budget, owner);
    }
    budget.with_deferred_refund_notifications(|_| {
        let mut writer = map
            .try_write_admitted(|demand| Policy::admit(&budget, &counters, demand, None))
            .unwrap();
        let original = writer.get(&7).unwrap().pointer();
        let (key, unused) = input(&budget, 7);
        drop(unused);
        let (missing, unused) = input(&budget, 999);
        drop(unused);
        let mut child = writer.checkpoint().unwrap();
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
            .unwrap();
        without_allocations(|| {
            assert_eq!(
                child.removal_demand(&missing).unwrap(),
                AllocationDemand::new()
            );
            assert!(
                child
                    .try_remove_admitted(&missing, |_| -> Result<Policy, ()> {
                        panic!("absent removal must not request admission")
                    })
                    .unwrap()
                    .is_none()
            );
        });
        let before = NEXT_RECORD.load(SeqCst);
        let error = without_allocations(|| {
            child.try_remove_admitted(&key, |demand| {
                Policy::admit(&budget, &counters, demand, None)
            })
        })
        .unwrap_err();
        assert!(matches!(
            error,
            MapAdmissionError::Refused(AllocationRefusal::Capacity { .. })
        ));
        assert_eq!(NEXT_RECORD.load(SeqCst), before);
        assert_eq!(child.get(&7).unwrap().pointer(), original);
        drop(blocker);
        let removed = child
            .try_remove_admitted(&key, |demand| {
                Policy::admit(&budget, &counters, demand, None)
            })
            .unwrap()
            .unwrap();
        assert_eq!(removed.order, 7);
        drop(removed);
        without_allocations(|| child.apply());
        assert!(writer.get(&7).is_none());
        without_allocations(|| writer.commit());
        drop(key);
        drop(missing);
        drop(map);
    });
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn admitted_removal_nested_abort_restores_original_nodes_at_full_capacity() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(16 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    for order in 0..80 {
        let owner = insert(&map, &budget, &counters, order);
        commit(&map, &budget, owner);
    }
    budget.with_deferred_refund_notifications(|_| {
        let mut writer = map
            .try_write_admitted(|demand| Policy::admit(&budget, &counters, demand, None))
            .unwrap();
        let original = writer.get(&0).unwrap().pointer();
        let baseline = budget.reserved_bytes();
        let start = NEXT_RECORD.load(SeqCst);
        let mut outer = writer.checkpoint().unwrap();
        let mut nested = outer.checkpoint().unwrap();
        for order in 0..80 {
            let (key, unused) = input(&budget, order);
            drop(unused);
            drop(
                nested
                    .try_remove_admitted(&key, |demand| {
                        Policy::admit(&budget, &counters, demand, None)
                    })
                    .unwrap()
                    .unwrap(),
            );
            drop(key);
        }
        assert!(nested.to_snapshot().is_empty());
        without_allocations(|| nested.apply());
        assert!(outer.to_snapshot().is_empty());
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
            .unwrap();
        let blocked = blocker.remaining_bytes();
        without_allocations(|| drop(outer));
        assert_eq!(writer.len(), 80);
        assert_eq!(writer.get(&0).unwrap().pointer(), original);
        assert_eq!(budget.reserved_bytes(), baseline + blocked);
        reclaimed_since(start);
        drop(blocker);
        without_allocations(|| writer.commit());
        drop(map);
    });
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn admitted_removal_clone_unwind_cannot_publish_and_refunds_private_copies() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for failure_position in 0..3 {
        reset();
        let budget = AllocationBudget::new(8 << 20);
        let counters = Arc::new(Counters::default());
        let map = map(&budget, &counters);
        for order in 0..80 {
            let owner = insert(&map, &budget, &counters, order);
            commit(&map, &budget, owner);
        }
        let original = map.read();
        let pointer = original.get(&0).unwrap().pointer();
        // Fanout changes the number of actual copies. Observe the same edit in
        // an aborted checkpoint first, then inject at its first, middle and last
        // copy so every supported tree shape reaches the requested crash cut.
        let copies = budget.with_deferred_refund_notifications(|_| {
            let mut writer = map
                .try_write_admitted(|demand| Policy::admit(&budget, &counters, demand, None))
                .unwrap();
            let (key, unused) = input(&budget, 0);
            drop(unused);
            let mut child = writer.checkpoint().unwrap();
            counters.keys.store(0, SeqCst);
            counters.values.store(0, SeqCst);
            drop(
                child
                    .try_remove_admitted(&key, |demand| {
                        Policy::admit(&budget, &counters, demand, None)
                    })
                    .unwrap()
                    .unwrap(),
            );
            let copies = counters.keys.load(SeqCst) + counters.values.load(SeqCst);
            without_allocations(|| drop(child));
            without_allocations(|| drop(writer));
            drop(key);
            copies
        });
        assert!(copies >= 3);
        let fail_at = match failure_position {
            0 => 1,
            1 => copies.div_ceil(2),
            _ => copies,
        };
        let baseline = budget.reserved_bytes();
        let start = NEXT_RECORD.load(SeqCst);
        budget.with_deferred_refund_notifications(|_| {
            let mut writer = map
                .try_write_admitted(|demand| Policy::admit(&budget, &counters, demand, None))
                .unwrap();
            let (key, unused) = input(&budget, 0);
            drop(unused);
            let mut child = writer.checkpoint().unwrap();
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = child.try_remove_admitted(&key, |demand| {
                        Policy::admit(&budget, &counters, demand, Some(fail_at))
                    });
                }))
                .is_err()
            );
            assert!(catch_unwind(AssertUnwindSafe(|| child.get(&0))).is_err());
            assert!(catch_unwind(AssertUnwindSafe(|| child.apply())).is_err());
            assert!(catch_unwind(AssertUnwindSafe(|| writer.commit())).is_err());
            drop(key);
        });
        assert_eq!(map.read().len(), 80);
        assert_eq!(map.read().get(&0).unwrap().pointer(), pointer);
        assert_eq!(budget.reserved_bytes(), baseline);
        reclaimed_since(start);
        budget.with_deferred_refund_notifications(|_| drop(original));
        budget.with_deferred_refund_notifications(|_| drop(map));
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

fn admitted_writer_start(map: &Map, budget: &AllocationBudget, counters: &Arc<Counters>) -> Owned {
    let start = NEXT_RECORD.load(SeqCst);
    let calls = counters.admissions.load(SeqCst);
    let keys = counters.keys.load(SeqCst);
    let values = counters.values.load(SeqCst);
    let mut required = None;
    let ((owner, bytes), allocations) = counted(|| {
        budget.with_deferred_refund_notifications(|_| {
            let writer = map
                .try_write_admitted(|demand| {
                    required = Some(demand);
                    Policy::admit(budget, counters, demand, None)
                })
                .unwrap_or_else(|error| panic!("admitted writer start refused: {error:?}"));
            (writer.detach(), required.unwrap().bytes())
        })
    });
    assert_eq!(required.unwrap().allocations(), 2);
    assert_eq!(
        allocations, 2,
        "only original cursor and next-reader shells allocate"
    );
    assert_eq!(NEXT_RECORD.load(SeqCst) - start, allocations);
    assert_eq!(counters.admissions.load(SeqCst), calls + 1);
    assert_eq!(counters.keys.load(SeqCst), keys);
    assert_eq!(counters.values.load(SeqCst), values);
    assert_eq!(
        RECORDS[start..NEXT_RECORD.load(SeqCst)]
            .iter()
            .map(|record| record.bytes.load(SeqCst))
            .sum::<usize>(),
        bytes
    );
    assert_live_credits(budget);
    owner
}

#[test]
fn admitted_empty_writer_starts_without_edits_and_grows_under_separate_admission() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let original = map.read();
    let mut owner = admitted_writer_start(&map, &budget, &counters);
    assert!(owner.to_snapshot().is_empty());
    assert!(map.read().is_empty());
    for order in 0..40 {
        let (key, value) = input(&budget, order);
        let pointer = value.pointer();
        let (next, previous) = edit(&map, &budget, &counters, owner, key, value);
        assert!(previous.is_none());
        assert_eq!(next.get(&order).unwrap().pointer(), pointer);
        assert!(map.read().is_empty());
        owner = next;
    }
    commit(&map, &budget, owner);
    assert_eq!(map.read().len(), 40);
    assert!(original.is_empty());
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(original)));
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn admitted_populated_writer_shares_original_entries_and_aborts_without_allocations() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let mut owner = insert(&map, &budget, &counters, 0);
    for order in 1..40 {
        let (key, value) = input(&budget, order);
        let (next, previous) = edit(&map, &budget, &counters, owner, key, value);
        assert!(previous.is_none());
        owner = next;
    }
    commit(&map, &budget, owner);
    let original = map.read();
    let before = budget.reserved_bytes();
    let owner = admitted_writer_start(&map, &budget, &counters);
    for order in 0..40 {
        assert_eq!(
            owner.get(&order).unwrap().pointer(),
            original.get(&order).unwrap().pointer()
        );
    }
    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
        .unwrap();
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(owner)));
    assert_eq!(original.len(), 40);
    assert_eq!(map.read().len(), 40);
    assert!(!map.is_poisoned());
    budget.with_deferred_refund_notifications(|_| drop(blocker));
    assert_eq!(budget.reserved_bytes(), before);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(original)));
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn admitted_writer_start_refuses_one_byte_below_and_accepts_exact_complete_demand() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let before = budget.reserved_bytes();
    let records = NEXT_RECORD.load(SeqCst);
    let mut required = None;
    let refused = without_allocations(|| {
        map.try_write_admitted(|demand| {
            required = Some(demand);
            Err::<Policy, ()>(())
        })
        .err()
        .expect("probe must refuse")
    });
    assert!(matches!(refused, MapAdmissionError::Refused(())));
    let demand = required.unwrap();
    assert_eq!(demand.allocations(), 2);
    assert_eq!(budget.reserved_bytes(), before);
    assert_eq!(NEXT_RECORD.load(SeqCst), records);
    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - before - demand.bytes() + 1)
        .unwrap();
    let held = budget.reserved_bytes();
    let calls = counters.admissions.load(SeqCst);
    let refused = without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| {
            map.try_write_admitted(|observed| {
                assert_eq!(observed, demand);
                Policy::admit(&budget, &counters, observed, None)
            })
            .err()
            .expect("one byte below must refuse")
        })
    });
    assert!(matches!(
        refused,
        MapAdmissionError::Refused(AllocationRefusal::Capacity { .. })
    ));
    assert_eq!(counters.admissions.load(SeqCst), calls + 1);
    assert_eq!(budget.reserved_bytes(), held);
    assert_eq!(NEXT_RECORD.load(SeqCst), records);
    assert!(map.read().is_empty());
    assert!(!map.is_poisoned());
    budget.with_deferred_refund_notifications(|_| drop(blocker));
    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - before - demand.bytes())
        .unwrap();
    let owner = budget.with_deferred_refund_notifications(|_| {
        map.try_write_admitted(|observed| {
            assert_eq!(observed, demand);
            Policy::admit(&budget, &counters, observed, None)
        })
        .unwrap_or_else(|error| panic!("exact demand refused: {error:?}"))
        .detach()
    });
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    assert_eq!(NEXT_RECORD.load(SeqCst), records + 2);
    assert_eq!(counters.keys.load(SeqCst), 0);
    assert_eq!(counters.values.load(SeqCst), 0);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(owner)));
    budget.with_deferred_refund_notifications(|_| drop(blocker));
    assert_eq!(budget.reserved_bytes(), before);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

// Proposed integration witnesses for the closed original current/undo insertion.
// This tests finite MV credits supplied to Concread; it is not a Storage adapter.
use concread::bptree::PairInsertError;

thread_local! {
    static PANIC_PAIR_UNDO_VALUE: Cell<bool> = const { Cell::new(false) };
}

impl NodeCloning<Payload, Option<Payload>> for Policy {
    fn clone_key(&mut self, key: &Payload) -> Payload {
        self.counters.keys.fetch_add(1, SeqCst);
        self.copy(key)
    }

    fn clone_value(&mut self, value: &Option<Payload>) -> Option<Payload> {
        value.as_ref().map(|value| {
            self.counters.values.fetch_add(1, SeqCst);
            let copied = self.copy(value);
            assert!(
                !PANIC_PAIR_UNDO_VALUE.with(|flag| flag.replace(false)),
                "injected actual undo payload clone panic"
            );
            copied
        })
    }
}

impl ClonePlanning<Payload, Option<Payload>> for Policy {
    fn plan_key(key: &Payload, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        demand.add_layout(key.layout())
    }

    fn plan_value(
        value: &Option<Payload>,
        demand: &mut AllocationDemand,
    ) -> Result<(), PlanningError> {
        if let Some(value) = value {
            demand.add_layout(value.layout())?;
        }
        Ok(())
    }
}

type UndoMap = BptreeMap<Payload, Option<Payload>, Prepaid<Policy>>;
type UndoOwned = BptreeMapOwned<Payload, Option<Payload>, Prepaid<Policy>>;

fn undo_map(budget: &AllocationBudget, counters: &Arc<Counters>) -> UndoMap {
    budget.with_deferred_refund_notifications(|_| {
        UndoMap::try_new_with_node_custody(|demand| Policy::admit(budget, counters, demand, None))
            .unwrap()
    })
}

fn undo_start(map: &UndoMap, budget: &AllocationBudget, counters: &Arc<Counters>) -> UndoOwned {
    budget.with_deferred_refund_notifications(|_| {
        map.try_write_admitted(|demand| Policy::admit(budget, counters, demand, None))
            .unwrap_or_else(|error| panic!("original undo start refused: {error:?}"))
            .detach()
    })
}

fn undo_commit(map: &UndoMap, budget: &AllocationBudget, owner: UndoOwned) {
    without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| {
            map.try_write_owned(owner)
                .unwrap_or_else(|_| panic!("original undo owner refused"))
                .commit();
        });
    });
}

fn pair_edit(
    maps: (&Map, &UndoMap),
    budget: &AllocationBudget,
    counters: &Arc<Counters>,
    owners: (Owned, UndoOwned),
    input: (Payload, Payload),
) -> ((Owned, UndoOwned), Option<Payload>) {
    let start = NEXT_RECORD.load(SeqCst);
    let calls = counters.admissions.load(SeqCst);
    let mut complete_demand = None;
    let (result, allocations) = counted(|| {
        budget.with_deferred_refund_notifications(|_| {
            maps.0
                .try_insert_with_undo_owned_admitted(
                    owners.0,
                    maps.1,
                    owners.1,
                    input.0,
                    input.1,
                    |demand| {
                        assert!(complete_demand.replace(demand).is_none());
                        Policy::admit(budget, counters, demand, None)
                    },
                )
                .unwrap_or_else(|(_, error)| panic!("joined insertion refused: {error:?}"))
        })
    });
    let complete_demand = complete_demand.unwrap();
    assert_eq!(counters.admissions.load(SeqCst), calls + 1);
    assert_eq!(allocations, NEXT_RECORD.load(SeqCst) - start);
    assert!(allocations <= complete_demand.allocations());
    let actual_bytes: usize = RECORDS[start..NEXT_RECORD.load(SeqCst)]
        .iter()
        .map(|record| record.bytes.load(SeqCst))
        .sum();
    assert!(actual_bytes <= complete_demand.bytes());
    assert_live_credits(budget);
    result
}

#[test]
fn pair_complete_demand_refusal_preserves_original_inputs_and_exact_budget_retry() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let current = map(&budget, &counters);
    let undo = undo_map(&budget, &counters);
    let published = insert(&current, &budget, &counters, 7);
    commit(&current, &budget, published);
    let old = current.read();
    let old_pointer = old.get(&7).unwrap().pointer();
    let baseline = budget.reserved_bytes();
    let private_start = NEXT_RECORD.load(SeqCst);
    let owner = admitted_writer_start(&current, &budget, &counters);
    let undo_owner = undo_start(&undo, &budget, &counters);
    let (key, mut value) = input(&budget, 7);
    value.bytes.fill(0xb7);
    let input_pointers = (key.pointer(), value.pointer());
    let clones = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
    let records = NEXT_RECORD.load(SeqCst);
    let before = budget.reserved_bytes();
    let mut planned = None;
    let mut probes = 0;
    let ((owner, undo_owner, key, value), error) = without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| {
            current
                .try_insert_with_undo_owned_admitted(
                    owner,
                    &undo,
                    undo_owner,
                    key,
                    value,
                    |demand| {
                        probes += 1;
                        planned = Some(demand);
                        Err::<Policy, ()>(())
                    },
                )
                .err()
                .expect("the complete probe must refuse before either edit")
        })
    });
    assert!(matches!(error, PairInsertError::Refused(())));
    assert_eq!(probes, 1);
    let demand = planned.unwrap();
    assert!(demand.bytes() > 0 && demand.allocations() > 0);
    assert_eq!((key.pointer(), value.pointer()), input_pointers);
    assert_eq!(owner.get(&7).unwrap().pointer(), old_pointer);
    assert!(undo_owner.to_snapshot().is_empty());
    assert_eq!(NEXT_RECORD.load(SeqCst), records);
    assert_eq!(budget.reserved_bytes(), before);
    assert_eq!(
        (counters.keys.load(SeqCst), counters.values.load(SeqCst)),
        clones
    );

    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - before - demand.bytes() + 1)
        .unwrap();
    let held = budget.reserved_bytes();
    let calls = counters.admissions.load(SeqCst);
    let ((owner, undo_owner, key, value), error) = without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| {
            current
                .try_insert_with_undo_owned_admitted(
                    owner,
                    &undo,
                    undo_owner,
                    key,
                    value,
                    |observed| {
                        assert_eq!(observed, demand);
                        Policy::admit(&budget, &counters, observed, None)
                    },
                )
                .err()
                .expect("one byte below the joined demand must refuse")
        })
    });
    let PairInsertError::Refused(AllocationRefusal::Capacity {
        requested_bytes, ..
    }) = error
    else {
        panic!("expected original joined capacity refusal");
    };
    assert_eq!(requested_bytes, demand.bytes());
    assert_eq!(counters.admissions.load(SeqCst), calls + 1);
    assert_eq!(budget.reserved_bytes(), held);
    assert_eq!(NEXT_RECORD.load(SeqCst), records);
    assert_eq!((key.pointer(), value.pointer()), input_pointers);
    assert_eq!(owner.get(&7).unwrap().pointer(), old_pointer);
    assert!(undo_owner.to_snapshot().is_empty());
    assert_eq!(
        (counters.keys.load(SeqCst), counters.values.load(SeqCst)),
        clones
    );
    budget.with_deferred_refund_notifications(|_| drop(blocker));
    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - before - demand.bytes())
        .unwrap();
    let calls = counters.admissions.load(SeqCst);
    let (((owner, undo_owner), previous), allocations) = counted(|| {
        budget.with_deferred_refund_notifications(|_| {
            current
                .try_insert_with_undo_owned_admitted(
                    owner,
                    &undo,
                    undo_owner,
                    key,
                    value,
                    |observed| {
                        assert_eq!(observed, demand);
                        Policy::admit(&budget, &counters, observed, None)
                    },
                )
                .unwrap_or_else(|(_, error)| panic!("exact joined budget refused: {error:?}"))
        })
    });
    assert_eq!(counters.admissions.load(SeqCst), calls + 1);
    assert_eq!(allocations, NEXT_RECORD.load(SeqCst) - records);
    assert!(allocations <= demand.allocations());
    assert_eq!(owner.get(&7).unwrap().pointer(), input_pointers.1);
    assert!(
        owner
            .get(&7)
            .unwrap()
            .bytes
            .iter()
            .all(|byte| *byte == 0xb7)
    );
    let first = undo_owner.get(&7).unwrap().as_ref().unwrap();
    assert!(first.bytes.iter().all(|byte| *byte == 7));
    assert_ne!(first.pointer(), old_pointer);
    assert_eq!(old.get(&7).unwrap().pointer(), old_pointer);
    assert_eq!(current.read().get(&7).unwrap().pointer(), old_pointer);
    assert!(undo.read().is_empty());
    budget.with_deferred_refund_notifications(|_| drop(blocker));
    assert_live_credits(&budget);
    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
        .unwrap();
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| drop((owner, undo_owner, previous)))
    });
    budget.with_deferred_refund_notifications(|_| drop(blocker));
    assert_eq!(budget.reserved_bytes(), baseline);
    reclaimed_since(private_start);
    assert!(!current.is_poisoned() && !undo.is_poisoned());
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(old)));
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop((current, undo))));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn pair_first_none_and_some_preimages_survive_replacement_growth_and_reader_custody() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(32 << 20);
    let counters = Arc::new(Counters::default());
    let current = map(&budget, &counters);
    let undo = undo_map(&budget, &counters);
    let published = insert(&current, &budget, &counters, 0);
    commit(&current, &budget, published);
    let old = current.read();
    let original_pointer = old.get(&0).unwrap().pointer();
    let original_id = old.get(&0).unwrap().id();
    let old_undo = undo.read();
    let mut owners = (
        admitted_writer_start(&current, &budget, &counters),
        undo_start(&undo, &budget, &counters),
    );
    for order in [0, 64] {
        let (key, mut value) = input(&budget, order);
        value.bytes.fill(0x91);
        let value_pointer = value.pointer();
        let (next, previous) =
            pair_edit((&current, &undo), &budget, &counters, owners, (key, value));
        assert_eq!(previous.is_some(), order == 0);
        assert_eq!(next.0.get(&order).unwrap().pointer(), value_pointer);
        budget.with_deferred_refund_notifications(|_| drop(previous));
        owners = next;
    }
    assert!(owners.1.get(&64).unwrap().is_none());
    let undo_before: [(usize, usize, Option<usize>); 2] = std::array::from_fn(|index| {
        let (key, value) = owners.1.iter().nth(index).unwrap();
        (
            key.order,
            key.pointer(),
            value.as_ref().map(Payload::pointer),
        )
    });
    for marker in [0xa1, 0xb2, 0xc3] {
        for order in [0, 64] {
            let (key, mut value) = input(&budget, order);
            value.bytes.fill(marker);
            let value_pointer = value.pointer();
            let (next, previous) =
                pair_edit((&current, &undo), &budget, &counters, owners, (key, value));
            assert!(previous.is_some());
            assert_eq!(next.0.get(&order).unwrap().pointer(), value_pointer);
            assert_eq!(next.1.to_snapshot().len(), 2);
            for ((key, value), expected) in next.1.iter().zip(undo_before) {
                assert_eq!(
                    (
                        key.order,
                        key.pointer(),
                        value.as_ref().map(Payload::pointer)
                    ),
                    expected
                );
            }
            assert!(
                next.1
                    .get(&0)
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .bytes
                    .iter()
                    .all(|byte| *byte == 0)
            );
            budget.with_deferred_refund_notifications(|_| drop(previous));
            owners = next;
        }
    }
    for order in 1..64 {
        let (next, previous) = pair_edit(
            (&current, &undo),
            &budget,
            &counters,
            owners,
            input(&budget, order),
        );
        assert!(previous.is_none());
        assert!(next.1.get(&order).unwrap().is_none());
        owners = next;
    }
    assert_eq!(owners.0.to_snapshot().len(), 65);
    assert_eq!(owners.1.to_snapshot().len(), 65);
    assert_eq!(current.read().len(), 1);
    assert!(undo.read().is_empty());
    // The pair API returns private owners. Each real existing publisher is
    // invoked explicitly; this does not invent an atomic MV publication API.
    commit(&current, &budget, owners.0);
    undo_commit(&undo, &budget, owners.1);
    assert_eq!(current.read().len(), 65);
    assert_eq!(undo.read().len(), 65);
    assert_eq!(old.get(&0).unwrap().pointer(), original_pointer);
    assert!(old_undo.is_empty());
    assert!(!RECORDS[original_id].freed.load(SeqCst));
    assert!(!RECORDS[original_id].refunded.load(SeqCst));
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop((old, old_undo))));
    assert!(RECORDS[original_id].freed.load(SeqCst));
    assert!(RECORDS[original_id].refunded.load(SeqCst));
    assert_live_credits(&budget);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop((current, undo))));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn pair_callback_and_nested_clone_panics_preserve_both_published_roots_and_reclaim_private_storage()
{
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for phase in 0..4 {
        reset();
        PANIC_PAIR_UNDO_VALUE.with(|flag| flag.set(false));
        let budget = AllocationBudget::new(8 << 20);
        let counters = Arc::new(Counters::default());
        let current = map(&budget, &counters);
        let undo = undo_map(&budget, &counters);
        for order in [0, 1] {
            let owner = insert(&current, &budget, &counters, order);
            commit(&current, &budget, owner);
        }
        let (key, value) = input(&budget, 99);
        let (undo_owner, replaced) = budget.with_deferred_refund_notifications(|_| {
            undo.try_insert_admitted(key, Some(value), |demand| {
                Policy::admit(&budget, &counters, demand, None)
            })
            .unwrap_or_else(|_| panic!("published original undo fixture"))
        });
        assert!(replaced.is_none());
        undo_commit(&undo, &budget, undo_owner);
        let old = current.read();
        let old_undo = undo.read();
        let current_pointer = old.get(&0).unwrap().pointer();
        let undo_pointer = old_undo.get(&99).unwrap().as_ref().unwrap().pointer();
        let baseline = budget.reserved_bytes();
        let start = NEXT_RECORD.load(SeqCst);
        let owners = (
            admitted_writer_start(&current, &budget, &counters),
            undo_start(&undo, &budget, &counters),
        );
        let (key, value) = input(&budget, 0);
        let mut callbacks = 0;
        PANIC_PAIR_UNDO_VALUE.with(|flag| flag.set(phase == 3));
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            budget.with_deferred_refund_notifications(|_| {
                let _ = current.try_insert_with_undo_owned_admitted(
                    owners.0,
                    &undo,
                    owners.1,
                    key,
                    value,
                    |demand| {
                        callbacks += 1;
                        assert_ne!(phase, 0, "injected joined callback panic");
                        // First-preimage key is copy1, its old value copy2,
                        // then the current leaf begins before any undo edit.
                        let fail_at = match phase {
                            1 => Some(1),
                            2 => Some(3),
                            _ => None,
                        };
                        Policy::admit(&budget, &counters, demand, fail_at)
                    },
                );
            });
        }));
        assert!(outcome.is_err(), "phase {phase} must actually unwind");
        assert_eq!(callbacks, 1);
        assert!(
            !PANIC_PAIR_UNDO_VALUE.with(|flag| flag.replace(false)),
            "undo payload fault was not reached"
        );
        assert!(current.is_poisoned() && undo.is_poisoned());
        assert_eq!(current.read().len(), 2);
        assert_eq!(undo.read().len(), 1);
        assert_eq!(current.read().get(&0).unwrap().pointer(), current_pointer);
        assert_eq!(
            undo.read().get(&99).unwrap().as_ref().unwrap().pointer(),
            undo_pointer
        );
        assert!(undo.read().get(&0).is_none());
        assert_eq!(budget.reserved_bytes(), baseline);
        reclaimed_since(start);
        assert_live_credits(&budget);
        without_allocations(|| {
            budget.with_deferred_refund_notifications(|_| drop((old, old_undo)))
        });
        without_allocations(|| {
            budget.with_deferred_refund_notifications(|_| drop((current, undo)))
        });
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn pair_foreign_and_busy_roles_return_original_nested_inputs_without_readmission() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let current = map(&budget, &counters);
    let undo = undo_map(&budget, &counters);
    let other_current = map(&budget, &counters);
    let other_undo = undo_map(&budget, &counters);
    let owner = insert(&current, &budget, &counters, 7);
    let (undo_key, undo_value) = input(&budget, 99);
    let (undo_owner, old) = budget.with_deferred_refund_notifications(|_| {
        undo.try_insert_admitted(undo_key, Some(undo_value), |demand| {
            Policy::admit(&budget, &counters, demand, None)
        })
        .unwrap_or_else(|_| panic!("original private undo"))
    });
    assert!(old.is_none());
    let (key, value) = input(&budget, 8);
    let pointers = (
        owner.get(&7).unwrap().pointer(),
        undo_owner.get(&99).unwrap().as_ref().unwrap().pointer(),
        key.pointer(),
        value.pointer(),
    );
    let assert_identity =
        |owner: &Owned, undo_owner: &UndoOwned, key: &Payload, value: &Payload| {
            assert_eq!(
                (
                    owner.get(&7).unwrap().pointer(),
                    undo_owner.get(&99).unwrap().as_ref().unwrap().pointer(),
                    key.pointer(),
                    value.pointer()
                ),
                pointers
            );
            assert_eq!(owner.to_snapshot().len(), 1);
            assert_eq!(undo_owner.to_snapshot().len(), 1);
            assert!(current.read().is_empty() && undo.read().is_empty());
        };
    let never = |_| -> Result<Policy, ()> { panic!("invalid owner must not reach pair admission") };
    let records = NEXT_RECORD.load(SeqCst);
    let calls = counters.admissions.load(SeqCst);
    let ((owner, undo_owner, key, value), error) = without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| {
            current
                .try_insert_with_undo_owned_admitted(
                    owner,
                    &other_undo,
                    undo_owner,
                    key,
                    value,
                    never,
                )
                .err()
                .expect("foreign undo owner")
        })
    });
    assert!(matches!(
        error,
        PairInsertError::Undo(OwnedWriteError::Changed)
    ));
    assert_identity(&owner, &undo_owner, &key, &value);
    let ((owner, undo_owner, key, value), error) = without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| {
            other_current
                .try_insert_with_undo_owned_admitted(owner, &undo, undo_owner, key, value, never)
                .err()
                .expect("foreign current owner")
        })
    });
    assert!(matches!(
        error,
        PairInsertError::Current(OwnedWriteError::Changed)
    ));
    assert_identity(&owner, &undo_owner, &key, &value);
    assert_eq!(NEXT_RECORD.load(SeqCst), records);
    assert_eq!(counters.admissions.load(SeqCst), calls);

    let competing_undo = undo_start(&undo, &budget, &counters);
    let records = NEXT_RECORD.load(SeqCst);
    let calls = counters.admissions.load(SeqCst);
    let (owner, undo_owner, key, value) = without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| {
            let held = undo
                .try_write_owned(competing_undo)
                .unwrap_or_else(|_| panic!("undo competitor"));
            let (inputs, error) = current
                .try_insert_with_undo_owned_admitted(owner, &undo, undo_owner, key, value, never)
                .err()
                .expect("busy undo owner");
            assert!(matches!(
                error,
                PairInsertError::Undo(OwnedWriteError::Busy)
            ));
            drop(held);
            inputs
        })
    });
    assert_identity(&owner, &undo_owner, &key, &value);
    assert_eq!(NEXT_RECORD.load(SeqCst), records);
    assert_eq!(counters.admissions.load(SeqCst), calls);
    let competing_current = admitted_writer_start(&current, &budget, &counters);
    let records = NEXT_RECORD.load(SeqCst);
    let calls = counters.admissions.load(SeqCst);
    let (owner, undo_owner, key, value) = without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| {
            let held = current
                .try_write_owned(competing_current)
                .unwrap_or_else(|_| panic!("current competitor"));
            let (inputs, error) = current
                .try_insert_with_undo_owned_admitted(owner, &undo, undo_owner, key, value, never)
                .err()
                .expect("busy current owner");
            assert!(matches!(
                error,
                PairInsertError::Current(OwnedWriteError::Busy)
            ));
            drop(held);
            inputs
        })
    });
    assert_identity(&owner, &undo_owner, &key, &value);
    assert_eq!(NEXT_RECORD.load(SeqCst), records);
    assert_eq!(counters.admissions.load(SeqCst), calls);
    let ((owner, undo_owner), previous) = pair_edit(
        (&current, &undo),
        &budget,
        &counters,
        (owner, undo_owner),
        (key, value),
    );
    assert!(previous.is_none());
    assert_eq!(owner.get(&8).unwrap().pointer(), pointers.3);
    assert!(undo_owner.get(&8).unwrap().is_none());
    without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| {
            drop((owner, undo_owner, current, undo, other_current, other_undo))
        })
    });
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[path = "admitted_map_custody/storage.rs"]
mod storage_custody;

fn prepaid_policy(reservation: AllocationReservation, counters: &Arc<Counters>) -> Policy {
    Policy {
        reservation,
        counters: Arc::clone(counters),
        copies: 0,
        fail_at: None,
    }
}

#[test]
fn prepared_checkpoint_cancel_and_exact_capacity_refusal_retain_original_input_and_root() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(8 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let owner = insert(&map, &budget, &counters, 0);
    let original = owner.get(&0).unwrap().pointer();
    let baseline = budget.reserved_bytes();
    budget.with_deferred_refund_notifications(|_| {
        let mut writer = map.try_write_owned(owner).unwrap_or_else(|_| panic!("original writer"));
        let mut checkpoint = writer.checkpoint().unwrap();
        let (key, value) = input(&budget, 7);
        let pointers = (key.pointer(), value.pointer());
        let copies = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
        let prepared = without_allocations(|| checkpoint.prepare_insert_admitted(key, value)
            .unwrap_or_else(|_| panic!("complete preparation")));
        let demand = prepared.demand();
        assert!(demand.bytes() > 0);
        let blocker = budget.try_reserve_bytes(
            budget.limit_bytes() - budget.reserved_bytes() - (demand.bytes() - 1),
        ).unwrap();
        let blocked = budget.reserved_bytes();
        let refusal = without_allocations(|| budget.try_reserve_bytes(demand.bytes())).unwrap_err();
        assert!(matches!(refusal, AllocationRefusal::Capacity { requested_bytes, .. } if requested_bytes == demand.bytes()));
        assert_eq!(budget.reserved_bytes(), blocked);
        assert_eq!(prepared.demand(), demand);
        let (key, value) = without_allocations(|| prepared.into_input());
        assert_eq!((key.pointer(), value.pointer()), pointers);
        assert_eq!((counters.keys.load(SeqCst), counters.values.load(SeqCst)), copies);
        assert_eq!(checkpoint.get(&0).unwrap().pointer(), original);
        assert_eq!(checkpoint.len(), 1);
        drop(blocker);
        let prepared = without_allocations(|| checkpoint.prepare_insert_admitted(key, value)
            .unwrap_or_else(|_| panic!("same original input retry")));
        assert_eq!(prepared.demand(), demand);
        let exact_blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes() - demand.bytes())
            .unwrap();
        let reservation = without_allocations(|| budget.try_reserve_bytes(demand.bytes()).unwrap());
        assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
        let start = NEXT_RECORD.load(SeqCst);
        let (previous, allocations) = counted(|| prepared.execute(prepaid_policy(reservation, &counters)));
        assert!(previous.is_none());
        assert_eq!(allocations, NEXT_RECORD.load(SeqCst) - start);
        assert!(allocations <= demand.allocations());
        assert!(counters.keys.load(SeqCst) > copies.0);
        assert!(counters.values.load(SeqCst) > copies.1);
        assert_eq!(checkpoint.get(&7).unwrap().pointer(), pointers.1);
        assert_eq!(checkpoint.get_before(&0).unwrap().pointer(), original);
        drop(exact_blocker);
        without_allocations(|| drop(checkpoint));
        assert_eq!(writer.get(&0).unwrap().pointer(), original);
        assert!(writer.get(&7).is_none());
        assert_eq!(budget.reserved_bytes(), baseline);
        without_allocations(|| writer.commit());
    });
    assert_eq!(map.read().get(&0).unwrap().pointer(), original);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn paired_preparations_share_one_reservation_and_preserve_independent_checkpoint_rollback() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(16 << 20);
    let counters = Arc::new(Counters::default());
    let left = map(&budget, &counters);
    let right = map(&budget, &counters);
    for order in 0..32 {
        commit(&left, &budget, insert(&left, &budget, &counters, order));
        commit(&right, &budget, insert(&right, &budget, &counters, order));
    }
    let left_old = left.read();
    let right_old = right.read();
    let left_original = left_old.get(&0).unwrap().pointer();
    let right_original = right_old.get(&0).unwrap().pointer();
    let left_owner = admitted_writer_start(&left, &budget, &counters);
    let right_owner = admitted_writer_start(&right, &budget, &counters);
    budget.with_deferred_refund_notifications(|_| {
        let mut left_writer = left
            .try_write_owned(left_owner)
            .unwrap_or_else(|_| panic!("left owner"));
        let mut right_writer = right
            .try_write_owned(right_owner)
            .unwrap_or_else(|_| panic!("right owner"));
        let mut left_child = left_writer.checkpoint().unwrap();
        let mut right_child = right_writer.checkpoint().unwrap();
        let (left_key, left_value) = input(&budget, 0);
        let (right_key, right_value) = input(&budget, 0);
        let left_new = left_value.pointer();
        let right_new = right_value.pointer();
        let left_prepared = without_allocations(|| {
            left_child
                .prepare_insert_admitted(left_key, left_value)
                .unwrap_or_else(|_| panic!("left prepared"))
        });
        let right_prepared = without_allocations(|| {
            right_child
                .prepare_insert_admitted(right_key, right_value)
                .unwrap_or_else(|_| panic!("right prepared"))
        });
        let left_demand = left_prepared.demand();
        let right_demand = right_prepared.demand();
        let combined = left_demand
            .bytes()
            .checked_add(right_demand.bytes())
            .unwrap();
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes() - combined)
            .unwrap();
        let mut original = without_allocations(|| budget.try_reserve_bytes(combined).unwrap());
        assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
        let left_reservation =
            without_allocations(|| original.try_partition_bytes(left_demand.bytes()).unwrap());
        let right_reservation =
            without_allocations(|| original.try_partition_bytes(right_demand.bytes()).unwrap());
        assert_eq!(original.remaining_bytes(), 0);
        assert!(matches!(
            without_allocations(|| budget.try_reserve_bytes(1)),
            Err(AllocationRefusal::Capacity { .. })
        ));
        let copies = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
        let start = NEXT_RECORD.load(SeqCst);
        let ((left_previous, right_previous), allocations) = counted(|| {
            (
                left_prepared.execute(prepaid_policy(left_reservation, &counters)),
                right_prepared.execute(prepaid_policy(right_reservation, &counters)),
            )
        });
        assert_eq!(allocations, NEXT_RECORD.load(SeqCst) - start);
        assert!(allocations <= left_demand.allocations() + right_demand.allocations());
        assert!(counters.keys.load(SeqCst) > copies.0);
        assert!(counters.values.load(SeqCst) > copies.1);
        drop((left_previous, right_previous, original, blocker));
        assert_eq!(left_child.get(&0).unwrap().pointer(), left_new);
        assert_eq!(right_child.get(&0).unwrap().pointer(), right_new);
        without_allocations(|| drop(left_child));
        without_allocations(|| right_child.apply());
        assert_eq!(left_writer.get(&0).unwrap().pointer(), left_original);
        assert_eq!(right_writer.get(&0).unwrap().pointer(), right_new);
        assert_eq!(right_old.get(&0).unwrap().pointer(), right_original);
        assert_live_credits(&budget);
        without_allocations(|| left_writer.commit());
        without_allocations(|| right_writer.commit());
        assert!(
            !RECORDS[right_old.get(&0).unwrap().id()]
                .refunded
                .load(SeqCst)
        );
    });
    assert_eq!(left.read().get(&0).unwrap().pointer(), left_original);
    assert_eq!(right_old.get(&0).unwrap().pointer(), right_original);
    assert_live_credits(&budget);
    without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| drop((left_old, right_old)))
    });
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop((left, right))));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn dropping_prepared_writer_input_never_edits_or_clones_the_original_cursor() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let counters = Arc::new(Counters::default());
    let map = map(&budget, &counters);
    let owner = insert(&map, &budget, &counters, 0);
    let original = owner.get(&0).unwrap().pointer();
    let baseline = budget.reserved_bytes();
    budget.with_deferred_refund_notifications(|_| {
        let mut writer = map
            .try_write_owned(owner)
            .unwrap_or_else(|_| panic!("original owner"));
        let start = NEXT_RECORD.load(SeqCst);
        let (key, value) = input(&budget, 7);
        let copies = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
        let prepared = without_allocations(|| {
            writer
                .prepare_insert_admitted(key, value)
                .unwrap_or_else(|_| panic!("writer preparation"))
        });
        without_allocations(|| drop(prepared));
        assert_eq!(
            (counters.keys.load(SeqCst), counters.values.load(SeqCst)),
            copies
        );
        assert_eq!(writer.get(&0).unwrap().pointer(), original);
        assert_eq!(writer.len(), 1);
        assert_eq!(budget.reserved_bytes(), baseline);
        reclaimed_since(start);
        without_allocations(|| writer.commit());
    });
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

impl NodeCloning<Payload, ()> for Policy {
    fn clone_key(&mut self, key: &Payload) -> Payload {
        self.counters.keys.fetch_add(1, SeqCst);
        self.copy(key)
    }
    fn clone_value(&mut self, (): &()) {}
}
impl ClonePlanning<Payload, ()> for Policy {
    fn plan_key(key: &Payload, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        demand.add_layout(key.layout())
    }
    fn plan_value((): &(), _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}

type TouchMap = BptreeMap<Payload, (), Prepaid<Policy>>;

#[test]
fn current_undo_and_touch_preparations_share_original_credit_and_abort_all_three_roots() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(16 << 20);
    let counters = Arc::new(Counters::default());
    let current = map(&budget, &counters);
    for order in 0..32 {
        commit(
            &current,
            &budget,
            insert(&current, &budget, &counters, order),
        );
    }
    let old = current.read();
    let original = old.get(&0).unwrap().pointer();
    let undo = budget.with_deferred_refund_notifications(|_| {
        UndoMap::try_new_with_node_custody(|d| Policy::admit(&budget, &counters, d, None)).unwrap()
    });
    let touch = budget.with_deferred_refund_notifications(|_| {
        TouchMap::try_new_with_node_custody(|d| Policy::admit(&budget, &counters, d, None)).unwrap()
    });
    budget.with_deferred_refund_notifications(|_| {
        let mut current_writer = current
            .try_write_admitted(|d| Policy::admit(&budget, &counters, d, None))
            .unwrap_or_else(|_| panic!("current writer"));
        let mut undo_writer = undo
            .try_write_admitted(|d| Policy::admit(&budget, &counters, d, None))
            .unwrap_or_else(|_| panic!("undo writer"));
        let mut touch_writer = touch
            .try_write_admitted(|d| Policy::admit(&budget, &counters, d, None))
            .unwrap_or_else(|_| panic!("touch writer"));
        let baseline = budget.reserved_bytes();
        let mut current_child = current_writer.checkpoint().unwrap();
        let mut undo_child = undo_writer.checkpoint().unwrap();
        let mut touch_child = touch_writer.checkpoint().unwrap();
        let (key, value) = input(&budget, 0);
        let incoming = (key.pointer(), value.pointer());
        let copies = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
        let cp = without_allocations(|| {
            current_child
                .prepare_insert_admitted(key, value)
                .unwrap_or_else(|_| panic!("current plan"))
        });
        assert_eq!(cp.input_key().pointer(), incoming.0);
        assert_eq!(cp.previous_value().unwrap().pointer(), original);
        let up = without_allocations(|| {
            undo_child
                .prepare_optional_copy_insert_admitted(cp.input_key(), cp.previous_value())
                .unwrap()
        });
        let tp = without_allocations(|| {
            touch_child
                .prepare_key_copy_insert_admitted(cp.input_key(), ())
                .unwrap_or_else(|_| panic!("touch plan"))
        });
        let demands = [cp.demand(), up.demand(), tp.demand()];
        let combined = demands
            .iter()
            .try_fold(0usize, |n, d| n.checked_add(d.bytes()))
            .unwrap();
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes() - combined + 1)
            .unwrap();
        let held = budget.reserved_bytes();
        assert!(matches!(
            without_allocations(|| budget.try_reserve_bytes(combined)),
            Err(AllocationRefusal::Capacity { .. })
        ));
        assert_eq!(budget.reserved_bytes(), held);
        assert_eq!(
            (counters.keys.load(SeqCst), counters.values.load(SeqCst)),
            copies
        );
        without_allocations(|| drop(up));
        without_allocations(|| tp.into_value());
        let (key, value) = without_allocations(|| cp.into_input());
        assert_eq!((key.pointer(), value.pointer()), incoming);
        assert_eq!(current_child.get(&0).unwrap().pointer(), original);
        assert!(undo_child.is_empty());
        assert!(touch_child.is_empty());
        drop(blocker);
        let cp = without_allocations(|| {
            current_child
                .prepare_insert_admitted(key, value)
                .unwrap_or_else(|_| panic!("same current plan"))
        });
        let up = without_allocations(|| {
            undo_child
                .prepare_optional_copy_insert_admitted(cp.input_key(), cp.previous_value())
                .unwrap()
        });
        let tp = without_allocations(|| {
            touch_child
                .prepare_key_copy_insert_admitted(cp.input_key(), ())
                .unwrap_or_else(|_| panic!("same touch plan"))
        });
        assert_eq!([cp.demand(), up.demand(), tp.demand()], demands);
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes() - combined)
            .unwrap();
        let mut reservation = without_allocations(|| budget.try_reserve_bytes(combined).unwrap());
        let current_funding = reservation.try_partition_bytes(demands[0].bytes()).unwrap();
        let undo_funding = reservation.try_partition_bytes(demands[1].bytes()).unwrap();
        let touch_funding = reservation.try_partition_bytes(demands[2].bytes()).unwrap();
        assert_eq!(reservation.remaining_bytes(), 0);
        assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
        assert!(matches!(
            budget.try_reserve_bytes(1),
            Err(AllocationRefusal::Capacity { .. })
        ));
        let records = NEXT_RECORD.load(SeqCst);
        let ((), copied_allocations) = counted(|| {
            assert!(
                up.execute(prepaid_policy(undo_funding, &counters))
                    .is_none()
            );
            assert!(
                tp.execute(prepaid_policy(touch_funding, &counters))
                    .is_none()
            );
        });
        // Finish dependent borrowed copies before moving their source owner.
        let (previous, insertion_allocations) = counted(|| {
            cp.execute(prepaid_policy(current_funding, &counters))
                .unwrap()
        });
        let allocations = copied_allocations + insertion_allocations;
        assert_eq!(allocations, NEXT_RECORD.load(SeqCst) - records);
        assert!(
            allocations
                <= demands
                    .iter()
                    .map(AllocationDemand::allocations)
                    .sum::<usize>()
        );
        assert_eq!(previous.order, 0);
        assert_eq!(current_child.get(&0).unwrap().pointer(), incoming.1);
        let preimage = undo_child.get(&0).unwrap().as_ref().unwrap();
        assert_eq!(&*preimage.bytes, &*old.get(&0).unwrap().bytes);
        assert_ne!(preimage.pointer(), original);
        assert_eq!(touch_child.get(&0), Some(&()));
        assert_eq!(old.get(&0).unwrap().pointer(), original);
        drop((reservation, blocker));
        assert_live_credits(&budget);
        drop(previous);
        without_allocations(|| drop((current_child, undo_child, touch_child)));
        assert_eq!(budget.reserved_bytes(), baseline);
        assert_eq!(current_writer.get(&0).unwrap().pointer(), original);
        assert!(undo_writer.is_empty());
        assert!(touch_writer.is_empty());
        without_allocations(|| drop((current_writer, undo_writer, touch_writer)));
    });
    assert_eq!(current.read().get(&0).unwrap().pointer(), original);
    assert!(undo.read().is_empty());
    assert!(touch.read().is_empty());
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop(old)));
    without_allocations(|| {
        budget.with_deferred_refund_notifications(|_| drop((current, undo, touch)))
    });
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn admitted_optional_none_is_retained_without_value_copy_and_survives_sibling_abort() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(4 << 20);
    let counters = Arc::new(Counters::default());
    let undo = budget.with_deferred_refund_notifications(|_| {
        UndoMap::try_new_with_node_custody(|d| Policy::admit(&budget, &counters, d, None)).unwrap()
    });
    let (key, value) = input(&budget, 7);
    budget.with_deferred_refund_notifications(|_| {
        let mut writer = undo
            .try_write_admitted(|d| Policy::admit(&budget, &counters, d, None))
            .unwrap_or_else(|_| panic!("undo writer"));
        let mut first = writer.checkpoint().unwrap();
        let prepared = without_allocations(|| {
            first
                .prepare_optional_copy_insert_admitted(&key, None)
                .unwrap()
        });
        let demand = prepared.demand();
        let keys = counters.keys.load(SeqCst);
        let values = counters.values.load(SeqCst);
        let records = NEXT_RECORD.load(SeqCst);
        let funding = budget.try_reserve_bytes(demand.bytes()).unwrap();
        let (previous, allocations) =
            counted(|| prepared.execute(prepaid_policy(funding, &counters)));
        assert!(previous.is_none());
        assert_eq!(allocations, NEXT_RECORD.load(SeqCst) - records);
        assert_eq!(counters.keys.load(SeqCst), keys + 1);
        assert_eq!(counters.values.load(SeqCst), values);
        assert!(matches!(first.get(&7), Some(None)));
        without_allocations(|| first.apply());
        let mut sibling = writer.checkpoint().unwrap();
        let prepared = without_allocations(|| {
            sibling
                .prepare_optional_copy_insert_admitted(&key, Some(&value))
                .unwrap()
        });
        let funding = budget.try_reserve_bytes(prepared.demand().bytes()).unwrap();
        assert!(matches!(
            prepared.execute(prepaid_policy(funding, &counters)),
            Some(None)
        ));
        assert!(matches!(sibling.get(&7), Some(Some(_))));
        assert!(matches!(sibling.get_before(&7), Some(None)));
        without_allocations(|| drop(sibling));
        assert!(matches!(writer.get(&7), Some(None)));
        without_allocations(|| writer.commit());
    });
    assert!(matches!(undo.read().get(&7), Some(None)));
    assert!(undo.read().get(&8).is_none());
    assert_live_credits(&budget);
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop((key, value, undo))));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn incoming_preimage_copy_unwind_reclaims_copies_and_poison_prevents_publication() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for fail_at in [1, 2] {
        reset();
        let budget = AllocationBudget::new(4 << 20);
        let counters = Arc::new(Counters::default());
        let undo = budget.with_deferred_refund_notifications(|_| {
            UndoMap::try_new_with_node_custody(|d| Policy::admit(&budget, &counters, d, None))
                .unwrap()
        });
        let (key, value) = input(&budget, 7);
        let pointers = (key.pointer(), value.pointer());
        let baseline = budget.reserved_bytes();
        let records = NEXT_RECORD.load(SeqCst);
        budget.with_deferred_refund_notifications(|_| {
            let mut writer = undo
                .try_write_admitted(|d| Policy::admit(&budget, &counters, d, None))
                .unwrap_or_else(|_| panic!("undo writer"));
            let mut child = writer.checkpoint().unwrap();
            let prepared = without_allocations(|| {
                child
                    .prepare_optional_copy_insert_admitted(&key, Some(&value))
                    .unwrap()
            });
            let provider =
                Policy::admit(&budget, &counters, prepared.demand(), Some(fail_at)).unwrap();
            assert!(catch_unwind(AssertUnwindSafe(|| prepared.execute(provider))).is_err());
            assert!(catch_unwind(AssertUnwindSafe(|| child.get(&7))).is_err());
            assert!(catch_unwind(AssertUnwindSafe(|| child.apply())).is_err());
            assert!(catch_unwind(AssertUnwindSafe(|| writer.commit())).is_err());
        });
        assert!(undo.is_poisoned());
        assert!(undo.read().is_empty());
        assert_eq!((key.pointer(), value.pointer()), pointers);
        assert_eq!(budget.reserved_bytes(), baseline);
        reclaimed_since(records);
        without_allocations(|| {
            budget.with_deferred_refund_notifications(|_| drop((key, value, undo)))
        });
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn copied_preimage_tracking_cleanup_panic_restores_original_parent_and_blocks_publication() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(4 << 20);
    let counters = Arc::new(Counters::default());
    let undo = budget.with_deferred_refund_notifications(|_| {
        UndoMap::try_new_with_node_custody(|d| Policy::admit(&budget, &counters, d, None)).unwrap()
    });
    let (key, value) = input(&budget, 7);
    budget.with_deferred_refund_notifications(|_| {
        let mut writer = undo
            .try_write_admitted(|d| Policy::admit(&budget, &counters, d, None))
            .unwrap_or_else(|_| panic!("undo writer"));
        let absent = writer
            .prepare_optional_copy_insert_admitted(&key, None)
            .unwrap();
        let funding = budget.try_reserve_bytes(absent.demand().bytes()).unwrap();
        assert!(absent.execute(prepaid_policy(funding, &counters)).is_none());
        assert!(matches!(writer.get(&7), Some(None)));
        let baseline = budget.reserved_bytes();
        let records = NEXT_RECORD.load(SeqCst);
        let mut child = writer.checkpoint().unwrap();
        let prepared = child
            .prepare_optional_copy_insert_admitted(&key, Some(&value))
            .unwrap();
        let funding = budget.try_reserve_bytes(prepared.demand().bytes()).unwrap();
        // The parent first_seen buffer holds one insertion. This child edit
        // needs three more slots and replaces that original tracking allocation.
        let first_buffer = NEXT_RECORD.load(SeqCst);
        assert!(matches!(
            prepared.execute(prepaid_policy(funding, &counters)),
            Some(None)
        ));
        PANIC_CHARGE.store(first_buffer, SeqCst);
        assert!(catch_unwind(AssertUnwindSafe(|| drop(child))).is_err());
        assert_eq!(PANIC_CHARGE.load(SeqCst), usize::MAX);
        assert_eq!(budget.reserved_bytes(), baseline);
        reclaimed_since(records);
        assert!(undo.read().is_empty());
        assert!(catch_unwind(AssertUnwindSafe(|| writer.get(&7))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| writer.commit())).is_err());
    });
    assert!(undo.is_poisoned());
    assert!(undo.read().is_empty());
    without_allocations(|| budget.with_deferred_refund_notifications(|_| drop((key, value, undo))));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}
