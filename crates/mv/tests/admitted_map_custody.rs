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
    budget.with_deferred_refund_notifications(|| {
        Map::try_new_with_node_custody(|demand| Policy::admit(budget, counters, demand, None))
            .unwrap()
    })
}

fn insert(map: &Map, budget: &AllocationBudget, counters: &Arc<Counters>, order: usize) -> Owned {
    let (key, value) = input(budget, order);
    let start = NEXT_RECORD.load(SeqCst);
    let mut planned_allocations = 0;
    let ((owner, previous), allocations) = counted(|| {
        budget.with_deferred_refund_notifications(|| {
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
        budget.with_deferred_refund_notifications(|| {
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
        budget.with_deferred_refund_notifications(|| {
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
    budget.with_deferred_refund_notifications(|| drop(blocking_credit));
    let (owner, old) = budget.with_deferred_refund_notifications(|| {
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
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(original)));
    assert!(RECORDS[old_id].freed.load(SeqCst));
    assert!(RECORDS[old_id].refunded.load(SeqCst));
    assert!(budget.reserved_bytes() < retained);
    assert_live_credits(&budget);
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
        budget.with_deferred_refund_notifications(|| {
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
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop((decoy, original))));
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
    let (owner, previous) = budget.with_deferred_refund_notifications(|| {
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
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
    // The detached cursor retains its real original root and base generation.
    // The returned value independently retains its actual nested allocation.
    assert!(!RECORDS[original_id].freed.load(SeqCst));
    assert!(!RECORDS[previous_id].freed.load(SeqCst));
    assert_eq!(owner.get(&7).unwrap().pointer(), replacement_pointer);
    assert!(previous.bytes.iter().all(|byte| *byte == 7));
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(owner)));
    assert!(RECORDS[original_id].freed.load(SeqCst));
    assert!(!RECORDS[previous_id].freed.load(SeqCst));
    assert_live_credits(&budget);
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(previous)));
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
            budget.with_deferred_refund_notifications(|| {
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
            budget.with_deferred_refund_notifications(|| {
                map.try_insert_admitted(key, value, |_| -> Result<Policy, ()> {
                    panic!("poisoned writer must not readmit")
                })
                .err()
                .expect("poison is explicit")
            })
        });
        assert!(matches!(error, MapAdmissionError::Poisoned));
        budget.with_deferred_refund_notifications(|| drop((key, value, original)));
        without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
        let (input, error) = self.budget.with_deferred_refund_notifications(|| {
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
    budget.with_deferred_refund_notifications(|| {
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
    budget.with_deferred_refund_notifications(|| drop((wait, waker, wake)));
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
        budget.with_deferred_refund_notifications(|| {
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
    budget.with_deferred_refund_notifications(|| drop(previous));
    commit(&map, &budget, owner);
    assert!(original.is_empty());
    let published = map.read();
    assert_eq!(published.len(), 128);
    assert_eq!(published.get(&7).unwrap().pointer(), replacement);
    without_allocations(|| {
        budget.with_deferred_refund_notifications(|| drop((published, original)))
    });
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
        budget.with_deferred_refund_notifications(|| {
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
    budget.with_deferred_refund_notifications(|| drop(blocker));
    let (owner, previous) = edit(&map, &budget, &counters, owner, key, value);
    assert!(previous.is_none());
    assert_eq!(owner.get(&7).unwrap().pointer(), pointers.1);
    commit(&map, &budget, owner);
    assert_eq!(map.read().len(), 2);
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
        budget.with_deferred_refund_notifications(|| {
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
        budget.with_deferred_refund_notifications(|| drop((owner, key, value, original, foreign)))
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
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(owner)));
    assert_eq!(budget.reserved_bytes(), baseline + blocked_bytes);
    assert_eq!(map.read().get(&255).unwrap().pointer(), original_pointer);
    assert_eq!(map.read().len(), 1);
    reclaimed_since(start);
    budget.with_deferred_refund_notifications(|| drop(blocker));
    assert_live_credits(&budget);
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
            budget.with_deferred_refund_notifications(|| {
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
        without_allocations(|| budget.with_deferred_refund_notifications(|| drop(old)));
        without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
    budget.with_deferred_refund_notifications(|| drop((owner, probe)));
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
            budget.with_deferred_refund_notifications(|| {
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
        without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
        budget.with_deferred_refund_notifications(|| {
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
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
    budget.with_deferred_refund_notifications(|| {
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
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
    budget.with_deferred_refund_notifications(|| {
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
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(old)));
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
        budget.with_deferred_refund_notifications(|| {
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
        without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
    budget.with_deferred_refund_notifications(|| {
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
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
    budget.with_deferred_refund_notifications(|| {
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
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

// These tests use the production MV pair coordinator and the same physical
// allocation/free witnesses as the public map controls above.
use mv::storage::{Storage, StorageAdmissionError, StorageReadOnly};
type AdmittedStorage = Storage<Payload, Payload, Prepaid<Policy>>;

fn component_policy(reservation: AllocationReservation, counters: &Arc<Counters>) -> Policy {
    Policy {
        reservation,
        counters: Arc::clone(counters),
        copies: 0,
        fail_at: None,
    }
}
fn storage(budget: &AllocationBudget, counters: &Arc<Counters>) -> AdmittedStorage {
    AdmittedStorage::try_new_with_node_custody(budget, |reservation| {
        component_policy(reservation, counters)
    })
    .unwrap()
}
fn seed_storage(
    storage: &AdmittedStorage,
    budget: &AllocationBudget,
    counters: &Arc<Counters>,
    count: usize,
) {
    budget.with_deferred_refund_notifications(|| {
        let mut block = storage
            .try_block_admitted(budget, |r| component_policy(r, counters))
            .unwrap();
        let mut transaction = block.try_transaction().unwrap();
        for order in 0..count {
            let (key, value) = input(budget, order);
            transaction
                .try_insert_admitted(key, value, budget, |r| component_policy(r, counters))
                .unwrap();
        }
        transaction.apply();
        block.commit();
    });
}

#[test]
fn prepaid_storage_reads_need_no_heap_credit_or_payload_copy() {
    fn scan(storage: &impl StorageReadOnly<Payload, Payload>, count: usize) {
        without_allocations(|| {
            let mut entries = storage.iter();
            let mut low = 0;
            let mut high = count;
            while low < high {
                assert_eq!(entries.len(), high - low);
                let (key, value) = if (high - low).is_multiple_of(2) {
                    high -= 1;
                    let entry = entries.next_back().unwrap();
                    assert_eq!(entry.0.order, high);
                    entry
                } else {
                    let entry = entries.next().unwrap();
                    assert_eq!(entry.0.order, low);
                    low += 1;
                    entry
                };
                assert_eq!(key.order, value.order);
            }
            assert_eq!(entries.len(), 0);
            assert!(entries.next().is_none());
            assert!(entries.next_back().is_none());
            let mut bounded = storage.range::<usize>((
                std::ops::Bound::Included(&7),
                std::ops::Bound::Excluded(&63),
            ));
            for order in (7..count.min(63)).rev() {
                let (key, value) = bounded.next_back().unwrap();
                assert_eq!(key.order, order);
                assert_eq!(value.order, order);
            }
            assert!(bounded.next().is_none());
            assert!(bounded.next_back().is_none());
        });
    }

    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 24);
    let counters = Arc::new(Counters::default());
    let storage = storage(&budget, &counters);
    scan(&storage.view(), 0);
    seed_storage(&storage, &budget, &counters, 65);
    let old = storage.view();
    let copied = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
    let charged = budget.reserved_bytes();
    // Exhaust the same pool: read traversal must not need even one new byte.
    let full = budget
        .try_reserve_bytes(budget.limit_bytes() - charged)
        .unwrap();
    scan(&old, 65);
    assert_eq!(
        (counters.keys.load(SeqCst), counters.values.load(SeqCst)),
        copied
    );
    drop(full);
    assert_eq!(budget.reserved_bytes(), charged);
    budget.with_deferred_refund_notifications(|| {
        let mut block = storage
            .try_block_admitted(&budget, |r| component_policy(r, &counters))
            .unwrap();
        scan(&block, 65);
        let mut transaction = block.try_transaction().unwrap();
        let (key, value) = input(&budget, 65);
        transaction
            .try_insert_admitted(key, value, &budget, |r| component_policy(r, &counters))
            .unwrap();
        scan(&transaction, 66);
        scan(&transaction.view(), 66);
        drop(transaction);
        scan(&block, 65);
        block.commit();
    });
    scan(&old, 65);
    scan(&storage.view(), 65);
    drop(old);
    drop(storage);
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn storage_joint_construction_refuses_before_any_original_tree_allocation() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(0);
    let error = without_allocations(|| {
        AdmittedStorage::try_new_with_node_custody(&budget, |_| {
            panic!("capacity refusal must precede both policy constructors")
        })
        .err()
        .unwrap()
    });
    assert!(matches!(
        error,
        StorageAdmissionError::Capacity(AllocationRefusal::ExceedsLimit { .. })
    ));
    assert_eq!(NEXT_RECORD.load(SeqCst), 0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn storage_block_refusal_preserves_both_original_trees_and_contention_wake() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let counters = Arc::new(Counters::default());
    let storage = storage(&budget, &counters);
    seed_storage(&storage, &budget, &counters, 12);
    let before = storage.view();
    let hold = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
        .unwrap();
    let error = without_allocations(|| {
        storage
            .try_block_admitted(&budget, |_| {
                panic!("whole pair refusal must precede any component allocation")
            })
            .err()
            .unwrap()
    });
    assert!(matches!(
        error,
        StorageAdmissionError::Capacity(AllocationRefusal::Capacity { .. })
    ));
    assert_eq!(before.len(), 12);
    drop(hold);
    budget.with_deferred_refund_notifications(|| {
        let block = storage
            .try_block_admitted(&budget, |r| component_policy(r, &counters))
            .unwrap();
        assert_eq!(block.len(), 12);
        assert_eq!(block.touched_entries().len(), 0);
        let error = without_allocations(|| {
            storage
                .try_block_admitted(&budget, |_| panic!("busy writer cannot admit"))
                .err()
                .unwrap()
        });
        let StorageAdmissionError::Busy(wait) = error else {
            panic!("original writer must supply its release observation")
        };
        let mut future = Box::pin(wait.wait_for_release());
        let mut context = Context::from_waker(Waker::noop());
        assert!(future.as_mut().poll(&mut context).is_pending());
        drop(block);
        assert!(future.as_mut().poll(&mut context).is_ready());
    });
    assert_eq!(before.get(&11).unwrap().order, 11);
    drop(before);
    drop(storage);
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn storage_insertion_reserves_whole_pair_before_copies_and_retries_original_inputs() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 22);
    let counters = Arc::new(Counters::default());
    let storage = storage(&budget, &counters);
    seed_storage(&storage, &budget, &counters, 24);
    let retained = storage.view();
    let original = retained.get(&7).unwrap().pointer();
    budget.with_deferred_refund_notifications(|| {
        let mut block = storage
            .try_block_admitted(&budget, |r| component_policy(r, &counters))
            .unwrap();
        let mut tx = without_allocations(|| block.try_transaction().unwrap());
        let (key, value) = input(&budget, 7);
        let input_pointers = (key.pointer(), value.pointer());
        let plan = without_allocations(|| tx.insertion_demand(&key).unwrap());
        let held_bytes = budget.limit_bytes() - budget.reserved_bytes() - (plan.bytes() - 1);
        let hold = budget.try_reserve_bytes(held_bytes).unwrap();
        let credits = budget.reserved_bytes();
        let copies = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
        let ((key, value), error) = without_allocations(|| {
            tx.try_insert_admitted(key, value, &budget, |_| {
                panic!("joint refusal precedes policy construction")
            })
            .unwrap_err()
        });
        let StorageAdmissionError::Capacity(AllocationRefusal::Capacity {
            requested_bytes, ..
        }) = error
        else {
            panic!("one whole-demand refusal")
        };
        assert_eq!(requested_bytes, plan.bytes());
        assert_eq!(budget.reserved_bytes(), credits);
        assert_eq!(
            copies,
            (counters.keys.load(SeqCst), counters.values.load(SeqCst))
        );
        assert_eq!((key.pointer(), value.pointer()), input_pointers);
        assert_eq!(tx.get(&7).unwrap().pointer(), original);
        assert_eq!(tx.touched_entries().len(), 0);
        drop(hold);
        let first = NEXT_RECORD.load(SeqCst);
        let (previous, allocations) = counted(|| {
            tx.try_insert_admitted(key, value, &budget, |r| component_policy(r, &counters))
                .unwrap()
        });
        assert_eq!(
            allocations,
            NEXT_RECORD.load(SeqCst) - first,
            "each insertion allocation must own its exact charge"
        );
        assert!(allocations <= plan.allocations());
        assert_eq!(previous.as_ref().unwrap().order, 7);
        drop(previous);
        assert_eq!(tx.get(&7).unwrap().pointer(), input_pointers.1);
        assert_eq!(
            tx.get_before_transaction(tx.touched_entries().next().unwrap().key)
                .unwrap()
                .pointer(),
            original
        );
        let before_block = tx
            .get_before_block(tx.touched_entries().next().unwrap().key)
            .unwrap()
            .pointer();
        let (key, value) = input(&budget, 7);
        let keys_before = counters.keys.load(SeqCst);
        let previous = tx
            .try_insert_admitted(key, value, &budget, |r| component_policy(r, &counters))
            .unwrap();
        drop(previous);
        assert_eq!(
            counters.keys.load(SeqCst),
            keys_before,
            "existing touch and first undo need no duplicate keys on the private leaf"
        );
        assert_eq!(tx.touched_entries().len(), 1);
        assert_eq!(
            tx.get_before_block(tx.touched_entries().next().unwrap().key)
                .unwrap()
                .pointer(),
            before_block
        );
        without_allocations(|| tx.apply());
        let entry = block.touched_entries().next().unwrap();
        assert_eq!(entry.before.unwrap().pointer(), before_block);
        block.commit();
    });
    assert_eq!(retained.get(&7).unwrap().pointer(), original);
    assert_live_credits(&budget);
    drop(retained);
    drop(storage);
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn storage_charged_splits_abort_without_allocation_and_preserve_sibling_first_absence() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 24);
    let counters = Arc::new(Counters::default());
    let storage = storage(&budget, &counters);
    budget.with_deferred_refund_notifications(|| {
        let mut block = storage
            .try_block_admitted(&budget, |r| component_policy(r, &counters))
            .unwrap();
        let mut first = block.try_transaction().unwrap();
        let (key, value) = input(&budget, 7);
        first
            .try_insert_admitted(key, value, &budget, |r| component_policy(r, &counters))
            .unwrap();
        first.apply();
        let original_pointer = block.get(&7).unwrap().pointer();
        let baseline = budget.reserved_bytes();
        let mut tx = block.try_transaction().unwrap();
        for order in (0..360).rev() {
            let (key, value) = input(&budget, order);
            let start = NEXT_RECORD.load(SeqCst);
            let (previous, allocations) = counted(|| {
                tx.try_insert_admitted(key, value, &budget, |r| component_policy(r, &counters))
                    .unwrap()
            });
            assert_eq!(allocations, NEXT_RECORD.load(SeqCst) - start);
            drop(previous);
        }
        assert_eq!(tx.touched_entries().len(), 360);
        for (order, entry) in tx.touched_entries().enumerate() {
            assert_eq!(entry.key.order, order);
            assert_eq!(entry.before.is_some(), order == 7);
            assert_eq!(entry.after.unwrap().order, order);
        }
        without_allocations(|| drop(tx));
        assert_eq!(budget.reserved_bytes(), baseline);
        assert_eq!(block.len(), 1);
        assert_eq!(block.get(&7).unwrap().pointer(), original_pointer);
        let mut sibling = block.try_transaction().unwrap();
        let (key, value) = input(&budget, 7);
        sibling
            .try_insert_admitted(key, value, &budget, |r| component_policy(r, &counters))
            .unwrap();
        let entry = sibling.touched_entries().next().unwrap();
        assert_eq!(entry.before.unwrap().pointer(), original_pointer);
        assert!(
            sibling.get_before_block(entry.key).is_none(),
            "Some(None) is the original absence, not a missing undo record"
        );
        sibling.apply();
        assert!(block.touched_entries().next().unwrap().before.is_none());
        block.commit();
    });
    assert_eq!(storage.view().len(), 1);
    drop(storage);
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn storage_clone_unwind_cannot_apply_part_of_the_original_pair() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for failed_component in [1, 3] {
        reset();
        let budget = AllocationBudget::new(1 << 22);
        let counters = Arc::new(Counters::default());
        let storage = storage(&budget, &counters);
        seed_storage(&storage, &budget, &counters, 20);
        let view = storage.view();
        let original = view.get(&7).unwrap().pointer();
        budget.with_deferred_refund_notifications(|| {
            let mut block = storage
                .try_block_admitted(&budget, |r| component_policy(r, &counters))
                .unwrap();
            let mut tx = block.try_transaction().unwrap();
            let (key, value) = input(&budget, 7);
            let mut component = 0;
            let failed = catch_unwind(AssertUnwindSafe(|| {
                tx.try_insert_admitted(key, value, &budget, |r| {
                    component += 1;
                    let mut policy = component_policy(r, &counters);
                    if component == failed_component {
                        policy.fail_at = Some(1);
                    }
                    policy
                })
                .unwrap();
            }));
            assert!(failed.is_err(), "injected payload clone must execute");
            assert_eq!(component, failed_component);
            assert!(catch_unwind(AssertUnwindSafe(|| tx.apply())).is_err());
            // No failed private execution acquires publication authority.
            assert_eq!(view.get(&7).unwrap().pointer(), original);
            drop(block);
        });
        assert_eq!(storage.view().get(&7).unwrap().pointer(), original);
        drop(view);
        drop(storage);
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn storage_touched_key_destructor_unwind_drains_original_buffer_before_refund() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 22);
    let counters = Arc::new(Counters::default());
    let storage = storage(&budget, &counters);
    budget.with_deferred_refund_notifications(|| {
        let mut block = storage
            .try_block_admitted(&budget, |r| component_policy(r, &counters))
            .unwrap();
        let mut tx = block.try_transaction().unwrap();
        for order in 0..9 {
            let (key, value) = input(&budget, order);
            tx.try_insert_admitted(key, value, &budget, |r| component_policy(r, &counters))
                .unwrap();
        }
        PANIC_CHARGE.store(tx.touched_entries().next_back().unwrap().key.id(), SeqCst);
        assert!(catch_unwind(AssertUnwindSafe(|| tx.apply())).is_err());
        assert_eq!(PANIC_CHARGE.load(SeqCst), usize::MAX);
        drop(block);
    });
    assert!(storage.view().is_empty());
    drop(storage);
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn storage_pair_apply_retirement_panic_forbids_parent_publication() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    // Both current and undo retain original nonempty tracking storage after
    // the first sibling. Exercise cleanup in each half of the aggregate pair.
    for choose_last in [false, true] {
        reset();
        let budget = AllocationBudget::new(1 << 24);
        let counters = Arc::new(Counters::default());
        let storage = storage(&budget, &counters);
        budget.with_deferred_refund_notifications(|| {
            let mut block = storage
                .try_block_admitted(&budget, |r| component_policy(r, &counters))
                .unwrap();
            let mut first = block.try_transaction().unwrap();
            let (key, value) = input(&budget, 7);
            first
                .try_insert_admitted(key, value, &budget, |r| component_policy(r, &counters))
                .unwrap();
            first.apply();
            let candidates: Vec<_> = RECORDS[..NEXT_RECORD.load(SeqCst)]
                .iter()
                .enumerate()
                .filter(|(_, record)| {
                    !record.freed.load(SeqCst)
                        && record.alignment.load(SeqCst) == std::mem::align_of::<usize>()
                        && record.bytes.load(SeqCst) <= 32
                })
                .map(|(id, _)| id)
                .collect();
            assert!(
                candidates.len() >= 2,
                "original current and undo tracking allocations"
            );
            let target = if choose_last {
                *candidates.last().unwrap()
            } else {
                candidates[0]
            };
            let mut tx = block.try_transaction().unwrap();
            for order in 0..160 {
                let (key, value) = input(&budget, order);
                tx.try_insert_admitted(key, value, &budget, |r| component_policy(r, &counters))
                    .unwrap();
            }
            assert!(
                !RECORDS[target].freed.load(SeqCst),
                "checkpoint retains its exact rollback allocation"
            );
            PANIC_CHARGE.store(target, SeqCst);
            assert!(catch_unwind(AssertUnwindSafe(|| tx.apply())).is_err());
            assert_eq!(
                PANIC_CHARGE.load(SeqCst),
                usize::MAX,
                "original retired bookkeeping destructor ran"
            );
            assert!(RECORDS[target].freed.load(SeqCst));
            assert!(catch_unwind(AssertUnwindSafe(|| block.get(&7))).is_err());
            assert!(catch_unwind(AssertUnwindSafe(|| block.commit())).is_err());
        });
        assert!(storage.view().is_empty());
        drop(storage);
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn storage_touched_buffer_growth_panic_preserves_single_custody_of_moved_keys() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 22);
    let counters = Arc::new(Counters::default());
    let storage = storage(&budget, &counters);
    budget.with_deferred_refund_notifications(|| {
        let mut block = storage
            .try_block_admitted(&budget, |r| component_policy(r, &counters))
            .unwrap();
        let mut tx = block.try_transaction().unwrap();
        for order in 0..4 {
            let (key, value) = input(&budget, order);
            tx.try_insert_admitted(key, value, &budget, |r| component_policy(r, &counters))
                .unwrap();
        }
        let layout = Layout::array::<Payload>(4).unwrap();
        let target = RECORDS[..NEXT_RECORD.load(SeqCst)]
            .iter()
            .position(|record| {
                !record.freed.load(SeqCst)
                    && record.bytes.load(SeqCst) == layout.size()
                    && record.alignment.load(SeqCst) == layout.align()
            })
            .expect("original four-key allocation");
        let (key, value) = input(&budget, 4);
        PANIC_CHARGE.store(target, SeqCst);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                tx.try_insert_admitted(key, value, &budget, |r| component_policy(r, &counters))
                    .unwrap();
            }))
            .is_err()
        );
        assert_eq!(PANIC_CHARGE.load(SeqCst), usize::MAX);
        assert!(catch_unwind(AssertUnwindSafe(|| tx.apply())).is_err());
        drop(block);
    });
    assert!(storage.view().is_empty());
    drop(storage);
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
        budget.with_deferred_refund_notifications(|| {
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
        without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
    budget.with_deferred_refund_notifications(|| {
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
    budget.with_deferred_refund_notifications(|| {
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
        let copies = budget.with_deferred_refund_notifications(|| {
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
        budget.with_deferred_refund_notifications(|| {
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
        budget.with_deferred_refund_notifications(|| drop(original));
        budget.with_deferred_refund_notifications(|| drop(map));
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn storage_missing_removal_admits_first_absence_and_touch_before_remaining_clean() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 22);
    let counters = Arc::new(Counters::default());
    let storage = storage(&budget, &counters);
    budget.with_deferred_refund_notifications(|| {
        let (query, unused) = input(&budget, 77);
        drop(unused);
        let original_query = query.pointer();
        let mut block = storage.try_block_admitted(&budget, |r| component_policy(r, &counters)).unwrap();
        let mut tx = block.try_transaction().unwrap();
        let demand = without_allocations(|| tx.removal_demand(&query).unwrap());
        assert!(demand.bytes() > 0, "first absence and touch own real storage");
        let hold = budget.try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes() - (demand.bytes() - 1)).unwrap();
        let credits = budget.reserved_bytes();
        let copies = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
        let refusal = without_allocations(|| tx.try_remove_admitted(&query, &budget, |_| panic!("joint refusal precedes provider construction"))).unwrap_err();
        assert!(matches!(refusal, StorageAdmissionError::Capacity(AllocationRefusal::Capacity { requested_bytes, .. }) if requested_bytes == demand.bytes()));
        assert_eq!(budget.reserved_bytes(), credits);
        assert_eq!((counters.keys.load(SeqCst), counters.values.load(SeqCst)), copies);
        assert_eq!(query.pointer(), original_query);
        assert_eq!(tx.touched_entries().len(), 0);
        drop(hold);
        let first = NEXT_RECORD.load(SeqCst);
        let (removed, allocations) = counted(|| tx.try_remove_admitted(&query, &budget, |r| component_policy(r, &counters)).unwrap());
        assert!(removed.is_none());
        assert_eq!(allocations, NEXT_RECORD.load(SeqCst) - first);
        assert!(allocations <= demand.allocations());
        let entry = tx.touched_entries().next().unwrap();
        assert_eq!(entry.key.order, 77);
        assert!(entry.before.is_none() && entry.after.is_none());
        assert_eq!(tx.touched_entries().len(), 1);
        assert_eq!(without_allocations(|| tx.removal_demand(&query).unwrap()).bytes(), 0);
        without_allocations(|| assert!(tx.try_remove_admitted(&query, &budget, |r| component_policy(r, &counters)).unwrap().is_none()));
        without_allocations(|| tx.apply());
        assert!(!block.is_dirty());
        assert!(matches!(block.revert_map().get(&77), Some(None)));
        assert_eq!(block.touched_entries().len(), 1);
        block.commit();
        assert!(storage.view().is_empty());
        drop(query);
    });
    assert_live_credits(&budget);
    drop(storage);
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn storage_removal_reserves_the_whole_pair_and_retains_returned_value_after_abort() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 22);
    let counters = Arc::new(Counters::default());
    let storage = storage(&budget, &counters);
    seed_storage(&storage, &budget, &counters, 64);
    let view = storage.view();
    let original = view.get(&7).unwrap().pointer();
    budget.with_deferred_refund_notifications(|| {
        let (query, unused) = input(&budget, 7);
        drop(unused);
        let mut block = storage.try_block_admitted(&budget, |r| component_policy(r, &counters)).unwrap();
        let mut tx = block.try_transaction().unwrap();
        let demand = without_allocations(|| tx.removal_demand(&query).unwrap());
        let hold = budget.try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes() - (demand.bytes() - 1)).unwrap();
        let credits = budget.reserved_bytes();
        let copies = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
        let error = without_allocations(|| tx.try_remove_admitted(&query, &budget, |_| panic!("no partial-credit admission"))).unwrap_err();
        assert!(matches!(error, StorageAdmissionError::Capacity(AllocationRefusal::Capacity { requested_bytes, .. }) if requested_bytes == demand.bytes()));
        assert_eq!(budget.reserved_bytes(), credits);
        assert_eq!((counters.keys.load(SeqCst), counters.values.load(SeqCst)), copies);
        assert_eq!(tx.get(&7).unwrap().pointer(), original);
        assert_eq!(tx.touched_entries().len(), 0);
        drop(hold);
        let first = NEXT_RECORD.load(SeqCst);
        let (removed, allocations) = counted(|| tx.try_remove_admitted(&query, &budget, |r| component_policy(r, &counters)).unwrap());
        let removed = removed.expect("present value returned to original caller");
        let returned_id = removed.id();
        let returned_pointer = removed.pointer();
        assert_eq!(removed.order, 7);
        assert_eq!(allocations, NEXT_RECORD.load(SeqCst) - first);
        assert!(allocations <= demand.allocations());
        assert!(tx.get(&7).is_none());
        let entry = tx.touched_entries().next().unwrap();
        assert_eq!(entry.before.unwrap().pointer(), original);
        assert!(entry.after.is_none());
        assert_eq!(tx.get_before_block(&query).unwrap().order, 7);
        without_allocations(|| drop(tx));
        assert_eq!(block.get(&7).unwrap().pointer(), original);
        assert!(!block.is_dirty());
        without_allocations(|| drop(block));
        assert_eq!(removed.pointer(), returned_pointer);
        assert!(!RECORDS[returned_id].freed.load(SeqCst));
        without_allocations(|| drop(removed));
        assert!(RECORDS[returned_id].freed.load(SeqCst));
        assert!(RECORDS[returned_id].refunded.load(SeqCst));
        drop(query);
    });
    assert_eq!(view.get(&7).unwrap().pointer(), original);
    assert_live_credits(&budget);
    drop(view);
    drop(storage);
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn storage_removal_merges_abort_at_full_budget_and_preserve_sibling_first_absence() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 24);
    let counters = Arc::new(Counters::default());
    let storage = storage(&budget, &counters);
    seed_storage(&storage, &budget, &counters, 128);
    let retained = storage.view();
    let original = retained.get(&7).unwrap().pointer();
    budget.with_deferred_refund_notifications(|| {
        let mut block = storage
            .try_block_admitted(&budget, |r| component_policy(r, &counters))
            .unwrap();
        let mut first = block.try_transaction().unwrap();
        let (key, value) = input(&budget, 200);
        first
            .try_insert_admitted(key, value, &budget, |r| component_policy(r, &counters))
            .unwrap();
        first.apply();
        let inserted = block.get(&200).unwrap().pointer();
        let baseline = budget.reserved_bytes();
        let start = NEXT_RECORD.load(SeqCst);
        let mut tx = block.try_transaction().unwrap();
        for order in (0..128).rev().chain(std::iter::once(200)) {
            let (query, unused) = input(&budget, order);
            drop(unused);
            let demand = without_allocations(|| tx.removal_demand(&query).unwrap());
            let allocation_start = NEXT_RECORD.load(SeqCst);
            let (removed, allocations) = counted(|| {
                tx.try_remove_admitted(&query, &budget, |r| component_policy(r, &counters))
                    .unwrap()
            });
            assert_eq!(removed.as_ref().unwrap().order, order);
            assert!(allocations <= demand.allocations());
            assert_eq!(allocations, NEXT_RECORD.load(SeqCst) - allocation_start);
            drop(removed);
            drop(query);
        }
        assert!(tx.is_empty());
        assert_eq!(tx.touched_entries().len(), 129);
        let hold = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
            .unwrap();
        let held_bytes = hold.remaining_bytes();
        without_allocations(|| drop(tx));
        assert_eq!(budget.reserved_bytes(), baseline + held_bytes);
        reclaimed_since(start);
        assert_eq!(block.len(), 129);
        assert_eq!(block.get(&7).unwrap().pointer(), original);
        assert_eq!(block.get(&200).unwrap().pointer(), inserted);
        drop(hold);
        let mut sibling = block.try_transaction().unwrap();
        let (query, unused) = input(&budget, 200);
        drop(unused);
        let removed = sibling
            .try_remove_admitted(&query, &budget, |r| component_policy(r, &counters))
            .unwrap();
        assert_eq!(
            sibling.get_before_transaction(&query).unwrap().pointer(),
            inserted
        );
        assert!(
            sibling.get_before_block(&query).is_none(),
            "original Some(None) survives sibling deletion"
        );
        drop(removed);
        sibling.apply();
        assert!(matches!(block.revert_map().get(&200), Some(None)));
        assert!(block.get(&200).is_none());
        assert_eq!(block.touched_entries().len(), 1);
        block.commit();
        drop(query);
    });
    assert_eq!(retained.get(&7).unwrap().pointer(), original);
    assert!(retained.get(&200).is_none());
    assert_eq!(storage.view().len(), 128);
    drop(retained);
    drop(storage);
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn storage_removal_copy_panic_before_and_after_undo_forbids_partial_apply() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for failed_component in [1, 3] {
        reset();
        let budget = AllocationBudget::new(1 << 22);
        let counters = Arc::new(Counters::default());
        let storage = storage(&budget, &counters);
        seed_storage(&storage, &budget, &counters, 32);
        let view = storage.view();
        let original = view.get(&7).unwrap().pointer();
        budget.with_deferred_refund_notifications(|| {
            let mut block = storage
                .try_block_admitted(&budget, |r| component_policy(r, &counters))
                .unwrap();
            let (query, unused) = input(&budget, 7);
            drop(unused);
            let baseline = budget.reserved_bytes();
            let start = NEXT_RECORD.load(SeqCst);
            let mut tx = block.try_transaction().unwrap();
            let mut component = 0;
            let error = catch_unwind(AssertUnwindSafe(|| {
                tx.try_remove_admitted(&query, &budget, |r| {
                    component += 1;
                    let mut policy = component_policy(r, &counters);
                    if component == failed_component {
                        policy.fail_at = Some(1);
                    }
                    policy
                })
                .unwrap();
            }));
            assert!(error.is_err());
            assert_eq!(component, failed_component);
            assert!(catch_unwind(AssertUnwindSafe(|| tx.apply())).is_err());
            if failed_component == 1 {
                // The external witness copier failed before either map edit.
                assert_eq!(block.get(&7).unwrap().pointer(), original);
                assert!(!block.is_dirty());
            } else {
                // An in-engine clone panic remains failed after checkpoint
                // cleanup even though its original allocations are restored.
                assert!(catch_unwind(AssertUnwindSafe(|| block.get(&7))).is_err());
            }
            assert_eq!(budget.reserved_bytes(), baseline);
            reclaimed_since(start);
            drop(query);
            if failed_component == 1 {
                drop(block);
            } else {
                assert!(catch_unwind(AssertUnwindSafe(|| block.commit())).is_err());
            }
        });
        assert_eq!(view.get(&7).unwrap().pointer(), original);
        assert_eq!(storage.view().get(&7).unwrap().pointer(), original);
        drop(view);
        drop(storage);
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn storage_removal_factory_cleanup_panic_keeps_transaction_rollback_armed() {
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("injected completed removal factory cleanup");
        }
    }
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 22);
    let counters = Arc::new(Counters::default());
    let storage = storage(&budget, &counters);
    seed_storage(&storage, &budget, &counters, 24);
    let original = storage.view().get(&7).unwrap().pointer();
    budget.with_deferred_refund_notifications(|| {
        let mut block = storage
            .try_block_admitted(&budget, |r| component_policy(r, &counters))
            .unwrap();
        let (query, unused) = input(&budget, 7);
        drop(unused);
        let baseline = budget.reserved_bytes();
        let start = NEXT_RECORD.load(SeqCst);
        let mut tx = block.try_transaction().unwrap();
        let bomb = PanicOnDrop;
        let provider_counters = Arc::clone(&counters);
        let failed = catch_unwind(AssertUnwindSafe(|| {
            tx.try_remove_admitted(&query, &budget, move |r| {
                let _keep_original_cleanup = &bomb;
                component_policy(r, &provider_counters)
            })
            .unwrap();
        }));
        assert!(failed.is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| tx.get(&7))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| tx.apply())).is_err());
        assert_eq!(block.get(&7).unwrap().pointer(), original);
        assert!(!block.is_dirty());
        assert_eq!(budget.reserved_bytes(), baseline);
        reclaimed_since(start);
        drop(query);
        drop(block);
    });
    assert_eq!(storage.view().get(&7).unwrap().pointer(), original);
    drop(storage);
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn storage_missing_removal_planning_refusal_preserves_unmodified_witness_owners() {
    struct RejectMissingKey(Policy);
    impl NodeFunding for RejectMissingKey {
        type Charge = Charge;
        fn take_node_charge(&mut self, layout: Layout) -> Charge {
            self.0.take_node_charge(layout)
        }
    }
    impl NodeCloning<Payload, Payload> for RejectMissingKey {
        fn clone_key(&mut self, key: &Payload) -> Payload {
            self.0.clone_key(key)
        }
        fn clone_value(&mut self, value: &Payload) -> Payload {
            self.0.clone_value(value)
        }
    }
    impl ClonePlanning<Payload, Payload> for RejectMissingKey {
        fn plan_key(key: &Payload, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
            if key.order == 77 {
                return Err(PlanningError::UnsupportedPayload);
            }
            Policy::plan_key(key, demand)
        }
        fn plan_value(value: &Payload, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
            Policy::plan_value(value, demand)
        }
    }
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 22);
    let counters = Arc::new(Counters::default());
    let provider = |r| RejectMissingKey(component_policy(r, &counters));
    let storage =
        Storage::<Payload, Payload, Prepaid<RejectMissingKey>>::try_new_with_node_custody(
            &budget, provider,
        )
        .unwrap();
    budget.with_deferred_refund_notifications(|| {
        let mut block = storage.try_block_admitted(&budget, provider).unwrap();
        let mut tx = block.try_transaction().unwrap();
        let (query, unused) = input(&budget, 77);
        drop(unused);
        let credits = budget.reserved_bytes();
        let copies = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
        assert!(matches!(
            without_allocations(|| tx.removal_demand(&query)),
            Err(PlanningError::UnsupportedPayload)
        ));
        let error = without_allocations(|| {
            tx.try_remove_admitted(&query, &budget, |_| {
                panic!("unsupported first witness must not reach admission")
            })
        })
        .unwrap_err();
        assert!(matches!(
            error,
            StorageAdmissionError::Planning(PlanningError::UnsupportedPayload)
        ));
        assert_eq!(budget.reserved_bytes(), credits);
        assert_eq!(
            (counters.keys.load(SeqCst), counters.values.load(SeqCst)),
            copies
        );
        assert!(tx.is_empty());
        assert_eq!(tx.touched_entries().len(), 0);
        drop(query);
        let (query, unused) = input(&budget, 7);
        drop(unused);
        assert!(
            tx.try_remove_admitted(&query, &budget, provider)
                .unwrap()
                .is_none()
        );
        tx.apply();
        assert!(!block.is_dirty());
        assert!(block.revert_map().get(&77).is_none());
        assert!(matches!(block.revert_map().get(&7), Some(None)));
        block.commit();
        drop(query);
    });
    drop(storage);
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

fn admitted_writer_start(map: &Map, budget: &AllocationBudget, counters: &Arc<Counters>) -> Owned {
    let start = NEXT_RECORD.load(SeqCst);
    let calls = counters.admissions.load(SeqCst);
    let keys = counters.keys.load(SeqCst);
    let values = counters.values.load(SeqCst);
    let mut required = None;
    let ((owner, bytes), allocations) = counted(|| {
        budget.with_deferred_refund_notifications(|| {
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
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(original)));
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(owner)));
    assert_eq!(original.len(), 40);
    assert_eq!(map.read().len(), 40);
    assert!(!map.is_poisoned());
    budget.with_deferred_refund_notifications(|| drop(blocker));
    assert_eq!(budget.reserved_bytes(), before);
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(original)));
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
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
        budget.with_deferred_refund_notifications(|| {
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
    budget.with_deferred_refund_notifications(|| drop(blocker));
    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - before - demand.bytes())
        .unwrap();
    let owner = budget.with_deferred_refund_notifications(|| {
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
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(owner)));
    budget.with_deferred_refund_notifications(|| drop(blocker));
    assert_eq!(budget.reserved_bytes(), before);
    without_allocations(|| budget.with_deferred_refund_notifications(|| drop(map)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}
