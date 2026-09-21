//! Black-box ownership checks for detached synchronous B+ tree generations.
//!
//! Every key/value clone owns a distinct boxed payload and a tracked destructor.
//! Pointer and clone observations prove original payload/node custody without
//! pretending to measure unrelated allocator or reader-control bookkeeping.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    borrow::Borrow,
    cell::Cell,
    cmp::Ordering,
    collections::BTreeMap,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering::SeqCst},
    },
};

use concread::bptree::{BptreeMap, BptreeMapOwned, BptreeMapReadSnapshot, OwnedWriteError};

// Controls activate this counter on their own thread after fixture/writer
// allocation. Payload observers only update already allocated drop records.
thread_local! {
    static ALLOCATION_COUNT: Cell<Option<usize>> = const { Cell::new(None) };
    static ALLOCATION_SIZES: Cell<[usize; 16]> = const { Cell::new([0; 16]) };
    static ALLOCATION_LIFETIME: Cell<Option<AllocationLifetime>> = const { Cell::new(None) };
}

#[derive(Clone, Copy, Default, Debug)]
struct AllocationLifetime {
    allocations: usize,
    deallocations: usize,
    allocated_bytes: usize,
    deallocated_bytes: usize,
}

struct ObservedAllocator;

fn allocated_layout(size: usize) {
    let _ = ALLOCATION_LIFETIME.try_with(|lifetime| {
        if let Some(mut totals) = lifetime.get() {
            totals.allocations += 1;
            totals.allocated_bytes += size;
            lifetime.set(Some(totals));
        }
    });
}

fn deallocated_layout(size: usize) {
    let _ = ALLOCATION_LIFETIME.try_with(|lifetime| {
        if let Some(mut totals) = lifetime.get() {
            totals.deallocations += 1;
            totals.deallocated_bytes += size;
            lifetime.set(Some(totals));
        }
    });
}

fn allocated(size: usize) {
    let _ = ALLOCATION_COUNT.try_with(|count| {
        if let Some(previous) = count.get() {
            count.set(Some(previous.saturating_add(1)));
            if previous < 16 {
                let _ = ALLOCATION_SIZES.try_with(|sizes| {
                    let mut recorded = sizes.get();
                    recorded[previous] = size;
                    sizes.set(recorded);
                });
            }
        }
    });
}

unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocated(layout.size());
        // SAFETY: forward the original allocator contract unchanged to System.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            allocated_layout(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        allocated(layout.size());
        // SAFETY: preserve the caller's requested layout and zeroing contract.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            allocated_layout(layout.size());
        }
        pointer
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        allocated(size);
        // SAFETY: forward the same live allocation, old layout and new size.
        let resized = unsafe { System.realloc(pointer, layout, size) };
        if !resized.is_null() {
            // The successful resize replaces the requested layout, whether
            // System grows it in place or moves it. Failure retains the old one.
            deallocated_layout(layout.size());
            allocated_layout(size);
        }
        resized
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: forward the same allocation and layout. Record only after
        // System has actually freed it, rather than at a payload Drop hook.
        unsafe { System.dealloc(pointer, layout) };
        deallocated_layout(layout.size());
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

fn balanced_allocation_lifetime(operation: impl FnOnce()) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ALLOCATION_LIFETIME.with(|lifetime| lifetime.set(None));
        }
    }
    assert!(
        ALLOCATION_LIFETIME
            .with(|lifetime| lifetime.replace(Some(AllocationLifetime::default())))
            .is_none()
    );
    let reset = Reset;
    operation();
    let totals = ALLOCATION_LIFETIME
        .with(|lifetime| lifetime.replace(None))
        .unwrap();
    drop(reset);
    assert!(
        totals.allocations > 0,
        "fixture must allocate its actual tree"
    );
    assert_eq!(totals.allocations, totals.deallocations, "{totals:?}");
    assert_eq!(
        totals.allocated_bytes, totals.deallocated_bytes,
        "{totals:?}"
    );
}

fn without_allocations<T>(phase: &'static str, operation: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ALLOCATION_COUNT.with(|count| count.set(None));
        }
    }
    ALLOCATION_SIZES.with(|sizes| sizes.set([0; 16]));
    assert!(
        ALLOCATION_COUNT
            .with(|count| count.replace(Some(0)))
            .is_none()
    );
    let reset = Reset;
    let result = operation();
    let count = ALLOCATION_COUNT.with(|count| count.replace(None)).unwrap();
    drop(reset);
    let sizes = ALLOCATION_SIZES.with(Cell::get);
    assert_eq!(
        count, 0,
        "{phase}: retained successor handoff allocated on this thread; first allocation/reallocation sizes: {sizes:?}"
    );
    result
}

fn assert_storage_reads_without_allocation(
    storage: &impl mv::storage::StorageReadOnly<u64, u64>,
    expected: &BTreeMap<u64, u64>,
) {
    use std::ops::Bound::{Excluded, Included, Unbounded};
    without_allocations("borrowed State storage traversal", || {
        let mut actual = storage.iter();
        let mut reference = expected.iter();
        let mut reverse = false;
        loop {
            assert_eq!(actual.len(), reference.len());
            assert_eq!(actual.size_hint(), reference.size_hint());
            let (next, wanted) = if reverse {
                (actual.next_back(), reference.next_back())
            } else {
                (actual.next(), reference.next())
            };
            assert_eq!(next, wanted);
            if wanted.is_none() {
                break;
            }
            reverse = !reverse;
        }
        assert_eq!(actual.next_back(), None);
        assert_eq!(actual.next(), None);
        assert_eq!(storage.first_key_value(), expected.first_key_value());
        assert_eq!(storage.last_key_value(), expected.last_key_value());
        for bounds in [
            (Unbounded, Unbounded),
            (Included(0), Included(0)),
            (Excluded(2), Included(4099)),
            (Unbounded, Excluded(5)),
            (Included(7), Unbounded),
        ] {
            let mut actual = storage.range(bounds);
            let mut reference = expected.range(bounds);
            let mut reverse = true;
            loop {
                let (next, wanted) = if reverse {
                    (actual.next_back(), reference.next_back())
                } else {
                    (actual.next(), reference.next())
                };
                assert_eq!(next, wanted);
                if wanted.is_none() {
                    break;
                }
                reverse = !reverse;
            }
            assert_eq!(actual.next(), None);
            assert_eq!(actual.next_back(), None);
        }
    });
}

#[test]
fn storage_reads_allocate_nothing_across_retained_views_edits_and_rollback() {
    use mv::storage::Storage;
    for size in [0, 1, 7, 65, 257] {
        let expected: BTreeMap<u64, u64> = (0..size).map(|key| (key, key * 10)).collect();
        let storage: Storage<_, _> = expected.iter().map(|(&key, &value)| (key, value)).collect();
        let old = storage.view();
        assert_storage_reads_without_allocation(&old, &expected);
        let mut block = storage.block();
        let mut edited = expected.clone();
        for key in (0..size).step_by(3) {
            block.remove(key);
            edited.remove(&key);
        }
        block.insert(4099, 90);
        edited.insert(4099, 90);
        assert_storage_reads_without_allocation(&block, &edited);
        {
            let mut transaction = block.transaction();
            transaction.insert(5, 555);
            transaction.remove(4099);
            let mut changed = edited.clone();
            changed.insert(5, 555);
            changed.remove(&4099);
            assert_storage_reads_without_allocation(&transaction, &changed);
            assert_storage_reads_without_allocation(&transaction.view(), &changed);
        }
        assert_storage_reads_without_allocation(&block, &edited);
        block.commit();
        assert_storage_reads_without_allocation(&old, &expected);
        assert_storage_reads_without_allocation(&storage.view(), &edited);
        let snapshot = storage.snapshot();
        assert_storage_reads_without_allocation(snapshot.current(), &edited);
    }
}

#[test]
fn storage_history_and_borrowed_string_ranges_allocate_nothing() {
    use mv::storage::{Storage, StorageReadOnly};
    use std::ops::Bound::{Excluded, Included};
    let mut storage: Storage<u64, u64> = (0..257).map(|key| (key, key * 10)).collect();
    let mut block = storage.block();
    for key in (0..257).step_by(2) {
        block.remove(key);
    }
    block.insert(999, 1);
    block.commit();
    let history = storage.history();
    without_allocations("retained predecessor traversal", || {
        let mut before = history.iter_before_block();
        for key in 0..257 {
            assert_eq!(before.next(), Some((&key, &(key * 10))));
        }
        assert_eq!(before.next(), None);
    });
    let expected: BTreeMap<String, u64> =
        (0..257).map(|key| (format!("key-{key:04}"), key)).collect();
    let storage: Storage<_, _> = expected.clone().into_iter().collect();
    let view = storage.view();
    without_allocations("borrowed str range traversal", || {
        let bounds = (Included("key-0007"), Excluded("key-0210"));
        let mut actual = view.range::<str>(bounds);
        let mut reference = expected.range::<str, _>(bounds);
        for step in 0..203 {
            assert_eq!(
                if step % 2 == 0 {
                    actual.next()
                } else {
                    actual.next_back()
                },
                if step % 2 == 0 {
                    reference.next()
                } else {
                    reference.next_back()
                }
            );
        }
        assert_eq!(actual.next(), None);
        assert_eq!(actual.next_back(), None);
    });
}

#[derive(Default)]
struct Observations {
    next: AtomicUsize,
    key_clones: AtomicUsize,
    value_clones: AtomicUsize,
    drops: Mutex<BTreeMap<usize, usize>>,
}

impl Observations {
    fn register(&self) -> usize {
        let id = self.next.fetch_add(1, SeqCst);
        assert!(self.drops.lock().unwrap().insert(id, 0).is_none());
        id
    }

    fn record_drop(&self, id: usize) {
        let mut drops = self.drops.lock().unwrap();
        let count = drops.get_mut(&id).expect("registered original allocation");
        *count += 1;
        assert_eq!(*count, 1, "payload destroyed more than once");
    }

    fn dropped(&self, id: usize) -> usize {
        self.drops.lock().unwrap()[&id]
    }

    fn clones(&self) -> (usize, usize) {
        (self.key_clones.load(SeqCst), self.value_clones.load(SeqCst))
    }

    fn assert_released(&self) {
        let drops = self.drops.lock().unwrap();
        assert!(!drops.is_empty());
        assert!(
            drops.values().all(|count| *count == 1),
            "retained payloads: {drops:?}"
        );
    }
}

struct Key {
    data: Box<u64>,
    id: usize,
    observations: Arc<Observations>,
}

impl Key {
    fn new(value: u64, observations: &Arc<Observations>) -> Self {
        Self {
            data: Box::new(value),
            id: observations.register(),
            observations: Arc::clone(observations),
        }
    }
}

impl Clone for Key {
    fn clone(&self) -> Self {
        self.observations.key_clones.fetch_add(1, SeqCst);
        Self::new(*self.data, &self.observations)
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        self.observations.record_drop(self.id);
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.data, f)
    }
}

impl Borrow<u64> for Key {
    fn borrow(&self) -> &u64 {
        &self.data
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.data.cmp(&other.data)
    }
}

struct Value {
    data: Box<u64>,
    id: usize,
    observations: Arc<Observations>,
}

impl Value {
    fn new(value: u64, observations: &Arc<Observations>) -> Self {
        Self {
            data: Box::new(value),
            id: observations.register(),
            observations: Arc::clone(observations),
        }
    }
}

impl Clone for Value {
    fn clone(&self) -> Self {
        self.observations.value_clones.fetch_add(1, SeqCst);
        Self::new(*self.data, &self.observations)
    }
}

impl Drop for Value {
    fn drop(&mut self) {
        self.observations.record_drop(self.id);
    }
}

type Map = BptreeMap<Key, Value>;
type Owned = BptreeMapOwned<Key, Value>;

fn map(observations: &Arc<Observations>, count: u64) -> Map {
    Map::from_iter(
        (0..count).map(|n| (Key::new(n, observations), Value::new(n * 10, observations))),
    )
}

fn addresses(
    snapshot: BptreeMapReadSnapshot<'_, Key, Value>,
) -> Vec<(u64, usize, usize, usize, usize)> {
    snapshot
        .iter()
        .map(|(key, value)| {
            (
                *key.data,
                std::ptr::from_ref(key) as usize,
                std::ptr::from_ref(key.data.as_ref()) as usize,
                std::ptr::from_ref(value) as usize,
                std::ptr::from_ref(value.data.as_ref()) as usize,
            )
        })
        .collect()
}

fn image(snapshot: BptreeMapReadSnapshot<'_, Key, Value>) -> BTreeMap<u64, u64> {
    snapshot
        .iter()
        .map(|(key, value)| (*key.data, *value.data))
        .collect()
}

fn refuse(map: &Map, owner: Owned, expected: OwnedWriteError) -> Owned {
    match map.try_write_owned(owner) {
        Err((owner, error)) => {
            assert_eq!(error, expected);
            owner
        }
        Ok(_) => panic!("unexpected adoption for {expected:?}"),
    }
}

#[test]
fn original_payloads_survive_detach_busy_retry_abort_and_publication_without_clones() {
    let observations = Arc::new(Observations::default());
    let map = map(&observations, 128);
    let before = map.read();
    let mut writer = map.write();
    let _ = writer.insert(Key::new(127, &observations), Value::new(900, &observations));
    drop(writer.remove(&Key::new(7, &observations)));
    let _ = writer.insert(Key::new(512, &observations), Value::new(901, &observations));
    let original = addresses(writer.to_snapshot());
    let expected = image(writer.to_snapshot());
    let clones = observations.clones();
    let owner = writer.detach();
    assert_eq!(addresses(owner.to_snapshot()), original);
    assert_eq!(observations.clones(), clones);

    let blocker = map.write();
    let owner = refuse(&map, owner, OwnedWriteError::Busy);
    assert_eq!(addresses(owner.to_snapshot()), original);
    assert_eq!(observations.clones(), clones);
    drop(blocker);
    let writer = map
        .try_write_owned(owner)
        .unwrap_or_else(|_| panic!("same original generation"));
    assert_eq!(addresses(writer.to_snapshot()), original);
    let owner = writer.detach();
    assert_eq!(addresses(owner.to_snapshot()), original);
    assert_eq!(
        observations.clones(),
        clones,
        "abort-to-owner must not rebuild payloads"
    );
    let writer = map
        .try_write_owned(owner)
        .unwrap_or_else(|_| panic!("same owner retry"));
    writer.commit();
    assert_eq!(addresses(map.read().to_snapshot()), original);
    assert_eq!(image(map.read().to_snapshot()), expected);
    assert_eq!(
        observations.clones(),
        clones,
        "publication moves the original successor"
    );
    assert_eq!(*before.get(&127_u64).unwrap().data, 1270);
    assert!(before.get(&512_u64).is_none());
    assert!(before.get(&7_u64).is_some());
    drop(before);
    drop(map);
    observations.assert_released();
}

#[test]
fn foreign_stale_and_equal_content_aba_refusals_return_the_exact_original_owner() {
    let observations = Arc::new(Observations::default());
    let map = map(&observations, 64);
    let foreign = self::map(&observations, 64);
    let mut writer = map.write();
    let _ = writer.insert(Key::new(0, &observations), Value::new(99, &observations));
    let owner = writer.detach();
    let original = addresses(owner.to_snapshot());
    let clones = observations.clones();
    let owner = refuse(&foreign, owner, OwnedWriteError::Changed);
    assert_eq!(addresses(owner.to_snapshot()), original);
    assert_eq!(observations.clones(), clones);

    // Even an untouched committed writer is a different predecessor generation.
    map.write().commit();
    let owner = refuse(&map, owner, OwnedWriteError::Changed);
    assert_eq!(addresses(owner.to_snapshot()), original);
    for value in [1, 0] {
        let mut writer = map.write();
        let _ = writer.insert(Key::new(0, &observations), Value::new(value, &observations));
        writer.commit();
    }
    assert_eq!(
        image(map.read().to_snapshot()),
        image(foreign.read().to_snapshot())
    );
    let clones = observations.clones();
    let owner = refuse(&map, owner, OwnedWriteError::Changed);
    assert_eq!(addresses(owner.to_snapshot()), original);
    assert_eq!(observations.clones(), clones);
    assert_eq!(*owner.get(&0_u64).unwrap().data, 99);
    drop(owner);
    drop(map);
    drop(foreign);
    observations.assert_released();
}

#[test]
fn detached_owner_keeps_shared_nodes_after_source_drop_and_cross_thread_transfer() {
    fn assert_send_static<T: Send + 'static>() {}
    assert_send_static::<Owned>();
    let observations = Arc::new(Observations::default());
    let map = map(&observations, 256);
    let original_far = addresses(map.read().to_snapshot()).pop().unwrap();
    let mut writer = map.write();
    let _ = writer.insert(Key::new(0, &observations), Value::new(999, &observations));
    drop(writer.remove(&Key::new(1, &observations)));
    let owner = writer.detach();
    assert_eq!(
        addresses(owner.to_snapshot()).pop().unwrap(),
        original_far,
        "far leaf remains shared with the original root"
    );
    let far_id = owner.get(&255_u64).unwrap().id;
    let original = addresses(owner.to_snapshot());
    let clones = observations.clones();
    drop(map);
    assert_eq!(observations.dropped(far_id), 0);
    let owner = std::thread::spawn(move || {
        assert_eq!(addresses(owner.to_snapshot()), original);
        assert_eq!(*owner.get(&0_u64).unwrap().data, 999);
        assert!(owner.get(&1_u64).is_none());
        assert_eq!(*owner.get(&255_u64).unwrap().data, 2550);
        assert_eq!(owner.to_snapshot().len(), 255);
        let mut entries = owner.iter();
        assert_eq!(entries.len(), 255);
        assert_eq!(*entries.next().unwrap().1.data, 999);
        assert_eq!(*entries.next_back().unwrap().1.data, 2550);
        assert_eq!(entries.len(), 253);
        assert_eq!(entries.count(), 253);
        owner
    })
    .join()
    .unwrap();
    assert_eq!(observations.clones(), clones);
    assert_eq!(observations.dropped(far_id), 0);
    drop(owner);
    observations.assert_released();
}

#[test]
fn old_reader_chain_retains_removed_payloads_across_splits_abort_and_later_commits() {
    let observations = Arc::new(Observations::default());
    let map = map(&observations, 32);
    let first = map.read();
    let first_zero = first.get(&0_u64).unwrap().id;
    let mut writer = map.write();
    for n in 0..160 {
        let _ = writer.insert(
            Key::new(n, &observations),
            Value::new(n + 1000, &observations),
        );
    }
    let owner = writer.detach();
    map.try_write_owned(owner)
        .unwrap_or_else(|_| panic!("split generation"))
        .commit();
    let second = map.read();
    let second_zero = second.get(&0_u64).unwrap().id;

    let mut abandoned = map.write();
    let _ = abandoned.insert(
        Key::new(999, &observations),
        Value::new(9999, &observations),
    );
    let abandoned_id = abandoned.get(&999_u64).unwrap().id;
    let abandoned = abandoned.detach();
    assert_eq!(observations.dropped(abandoned_id), 0);
    drop(abandoned);
    assert_eq!(observations.dropped(abandoned_id), 1);
    assert!(map.read().get(&999_u64).is_none());

    let mut writer = map.write();
    for n in 0..150 {
        drop(writer.remove(&Key::new(n, &observations)));
    }
    for n in 300..450 {
        let _ = writer.insert(Key::new(n, &observations), Value::new(n * 2, &observations));
    }
    let expected = BTreeMap::from_iter(
        (150..160)
            .map(|n| (n, n + 1000))
            .chain((300..450).map(|n| (n, n * 2))),
    );
    assert_eq!(image(writer.to_snapshot()), expected);
    let owner = writer.detach();
    map.try_write_owned(owner)
        .unwrap_or_else(|_| panic!("removal generation"))
        .commit();
    let third = map.read();
    assert_eq!(image(third.to_snapshot()), expected);
    assert_eq!(*first.get(&0_u64).unwrap().data, 0);
    assert_eq!(*second.get(&0_u64).unwrap().data, 1000);
    assert_eq!(observations.dropped(first_zero), 0);
    assert_eq!(observations.dropped(second_zero), 0);
    drop(second);
    assert_eq!(
        observations.dropped(second_zero),
        0,
        "older reader pins the successor chain"
    );
    drop(first);
    assert_eq!(observations.dropped(first_zero), 1);
    assert_eq!(observations.dropped(second_zero), 1);
    assert_eq!(image(third.to_snapshot()), expected);
    drop(third);
    drop(map);
    observations.assert_released();
}

#[test]
fn final_map_destruction_allocates_nothing_and_frees_every_original_layout() {
    for entries in [0, 1, 7, 128, 4096] {
        // Include the observer itself so zero final balance witnesses actual
        // node/control/payload deallocation, not just payload Drop callbacks.
        balanced_allocation_lifetime(|| {
            let observations = Arc::new(Observations::default());
            let map = map(&observations, entries);
            without_allocations("final committed tree destruction", || drop(map));
            if entries != 0 {
                observations.assert_released();
            }
            drop(observations);
        });
    }
}

#[test]
fn retained_reader_chain_and_final_tree_reclamation_do_not_allocate() {
    balanced_allocation_lifetime(|| {
        let observations = Arc::new(Observations::default());
        let map = map(&observations, 512);
        let first = map.read();
        let first_id = first.get(&0_u64).unwrap().id;
        let mut writer = map.write();
        for n in 0..256 {
            drop(writer.insert(
                Key::new(n, &observations),
                Value::new(n + 1000, &observations),
            ));
        }
        writer.commit();
        let second = map.read();
        let second_id = second.get(&0_u64).unwrap().id;
        let mut writer = map.write();
        for n in 0..128 {
            drop(writer.remove(&Key::new(n, &observations)));
        }
        writer.commit();
        assert!(map.read().get(&0_u64).is_none());

        without_allocations("intermediate reader release", || drop(second));
        assert_eq!(observations.dropped(first_id), 0);
        assert_eq!(observations.dropped(second_id), 0);
        without_allocations("oldest reader and retired generation reclamation", || {
            drop(first)
        });
        assert_eq!(observations.dropped(first_id), 1);
        assert_eq!(observations.dropped(second_id), 1);
        without_allocations("remaining committed tree destruction", || drop(map));
        observations.assert_released();
        drop(observations);
    });
}

#[test]
fn final_detached_owner_reclaims_unpublished_nodes_and_retained_root_without_allocation() {
    balanced_allocation_lifetime(|| {
        let observations = Arc::new(Observations::default());
        let map = map(&observations, 1024);
        let shared_id = map.read().get(&1023_u64).unwrap().id;
        let mut writer = map.write();
        drop(writer.insert(Key::new(0, &observations), Value::new(9000, &observations)));
        drop(writer.remove(&Key::new(1, &observations)));
        drop(writer.insert(
            Key::new(4096, &observations),
            Value::new(9001, &observations),
        ));
        let unpublished_id = writer.get(&4096_u64).unwrap().id;
        let owner = writer.detach();
        without_allocations("source map release while original root is retained", || {
            drop(map)
        });
        assert_eq!(observations.dropped(shared_id), 0);
        assert_eq!(observations.dropped(unpublished_id), 0);
        assert_eq!(*owner.get(&1023_u64).unwrap().data, 10230);
        assert_eq!(*owner.get(&4096_u64).unwrap().data, 9001);

        // Existing owner field order aborts unpublished nodes before releasing
        // its exact base reader and finally the original shared SuperBlock.
        without_allocations("last detached owner and final root destruction", || {
            drop(owner)
        });
        assert_eq!(observations.dropped(shared_id), 1);
        assert_eq!(observations.dropped(unpublished_id), 1);
        observations.assert_released();
        drop(observations);
    });
}

#[test]
fn stale_detached_owner_reclaims_its_old_base_and_newer_committed_root_without_allocation() {
    balanced_allocation_lifetime(|| {
        let observations = Arc::new(Observations::default());
        let map = map(&observations, 512);
        let original_last = map.read().get(&511_u64).unwrap().id;
        let mut abandoned = map.write();
        drop(abandoned.insert(Key::new(0, &observations), Value::new(111, &observations)));
        let unpublished_id = abandoned.get(&0_u64).unwrap().id;
        let abandoned = abandoned.detach();

        let mut committed = map.write();
        drop(committed.insert(Key::new(511, &observations), Value::new(222, &observations)));
        let current_last = committed.get(&511_u64).unwrap().id;
        committed.commit();
        assert_ne!(original_last, current_last);
        assert_eq!(*map.read().get(&511_u64).unwrap().data, 222);
        assert_eq!(*abandoned.get(&511_u64).unwrap().data, 5110);
        without_allocations("map release with an older detached predecessor", || {
            drop(map)
        });
        assert_eq!(observations.dropped(original_last), 0);
        assert_eq!(observations.dropped(current_last), 0);
        assert_eq!(observations.dropped(unpublished_id), 0);

        without_allocations(
            "old detached predecessor and newer final root destruction",
            || {
                drop(abandoned);
            },
        );
        assert_eq!(observations.dropped(original_last), 1);
        assert_eq!(observations.dropped(current_last), 1);
        assert_eq!(observations.dropped(unpublished_id), 1);
        observations.assert_released();
        drop(observations);
    });
}

#[test]
fn sibling_candidate_cannot_adopt_after_another_commit_but_retains_its_shared_base() {
    let observations = Arc::new(Observations::default());
    let map = map(&observations, 256);
    let mut first = map.write();
    let _ = first.insert(Key::new(0, &observations), Value::new(111, &observations));
    let first = first.detach();
    let original = addresses(first.to_snapshot());
    let mut second = map.write();
    let _ = second.insert(Key::new(255, &observations), Value::new(222, &observations));
    let second = second.detach();
    map.try_write_owned(second)
        .unwrap_or_else(|_| panic!("second original candidate"))
        .commit();
    assert_eq!(*map.read().get(&255_u64).unwrap().data, 222);
    let clones = observations.clones();
    let first = refuse(&map, first, OwnedWriteError::Changed);
    assert_eq!(observations.clones(), clones);
    drop(map);
    assert_eq!(addresses(first.to_snapshot()), original);
    assert_eq!(*first.get(&0_u64).unwrap().data, 111);
    assert_eq!(*first.get(&255_u64).unwrap().data, 2550);
    drop(first);
    observations.assert_released();
}

#[test]
fn poisoned_writer_refuses_adoption_without_consuming_the_original_generation() {
    let observations = Arc::new(Observations::default());
    let map = map(&observations, 64);
    let mut writer = map.write();
    let _ = writer.insert(Key::new(0, &observations), Value::new(77, &observations));
    let owner = writer.detach();
    let original = addresses(owner.to_snapshot());
    let clones = observations.clones();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _writer = map.write();
            panic!("original physical writer was abandoned during unwind");
        }))
        .is_err()
    );
    assert!(map.is_poisoned());
    let owner = refuse(&map, owner, OwnedWriteError::Poisoned);
    assert_eq!(addresses(owner.to_snapshot()), original);
    assert_eq!(observations.clones(), clones);
    assert_eq!(*map.read().get(&0_u64).unwrap().data, 0);
    assert_eq!(*owner.get(&0_u64).unwrap().data, 77);
    drop(owner);
    drop(map);
    observations.assert_released();
}

#[test]
fn clear_successor_preserves_old_reader_until_its_exact_payloads_are_released() {
    let observations = Arc::new(Observations::default());
    let map = map(&observations, 96);
    let original = map.read();
    let zero = original.get(&0_u64).unwrap().id;
    let mut writer = map.write();
    writer.clear();
    let owner = writer.detach();
    assert!(owner.to_snapshot().is_empty());
    let clones = observations.clones();
    map.try_write_owned(owner)
        .unwrap_or_else(|_| panic!("original empty successor"))
        .commit();
    assert!(map.read().is_empty());
    assert_eq!(observations.clones(), clones);
    assert_eq!(*original.get(&95_u64).unwrap().data, 950);
    assert_eq!(observations.dropped(zero), 0);
    drop(original);
    observations.assert_released();
    drop(map);
    observations.assert_released();
}

#[test]
fn scalar_detach_contention_retry_abort_and_commit_allocate_no_new_successor() {
    let map = BptreeMap::from_iter((0_u64..256).map(|n| (n, n * 10)));
    let old = map.read();
    let mut writer = map.write();
    let _ = writer.insert(0, 77);
    let _ = writer.remove(&1);
    let _ = writer.insert(512, 88);
    let pointer = std::ptr::from_ref(writer.get(&0).unwrap());
    let owner = without_allocations("initial detach", || writer.detach());
    let blocker = map.write();
    let owner = match without_allocations("busy probe", || map.try_write_owned(owner)) {
        Err((owner, OwnedWriteError::Busy)) => owner,
        _ => panic!("original writer is physically held"),
    };
    drop(blocker);
    let writer = without_allocations("first adoption", || map.try_write_owned(owner))
        .unwrap_or_else(|_| panic!("original successor retry"));
    assert_eq!(std::ptr::from_ref(writer.get(&0).unwrap()), pointer);
    let owner = without_allocations("abort to owner", || writer.detach());
    let writer = without_allocations("retry adoption", || map.try_write_owned(owner))
        .unwrap_or_else(|_| panic!("original successor after abort"));
    without_allocations("commit", || writer.commit());
    assert_eq!(std::ptr::from_ref(map.read().get(&0).unwrap()), pointer);
    assert_eq!(old.get(&0), Some(&0));
    assert_eq!(old.get(&1), Some(&10));
    assert_eq!(map.read().get(&0), Some(&77));
    assert!(map.read().get(&1).is_none());
    assert_eq!(map.read().get(&512), Some(&88));
}

#[test]
fn fresh_map_first_commit_without_a_reader_allocates_no_new_successor() {
    let map = BptreeMap::<u64, u64>::new();
    let mut writer = map.write();
    for key in 0..64 {
        let _ = writer.insert(key, key * 3);
    }
    let pointer = std::ptr::from_ref(writer.get(&0).unwrap());
    // No map read or earlier commit may initialize the publication lock for us.
    let owner = without_allocations("cold initial detach", || writer.detach());
    let writer = without_allocations("cold first adoption", || map.try_write_owned(owner))
        .unwrap_or_else(|_| panic!("original first generation"));
    without_allocations("cold first commit", || writer.commit());
    let published = map.read();
    assert_eq!(published.len(), 64);
    assert_eq!(std::ptr::from_ref(published.get(&0).unwrap()), pointer);
    assert_eq!(published.get(&63), Some(&189));
}

#[test]
fn storage_transaction_abort_restores_both_parent_trees_without_allocating_or_cloning() {
    use mv::storage::{Storage, StorageReadOnly};

    let observations = Arc::new(Observations::default());
    let storage: Storage<Key, Value> = (0..128)
        .map(|key| {
            (
                Key::new(key, &observations),
                Value::new(key * 10, &observations),
            )
        })
        .collect();
    let old = storage.view();
    let mut block = storage.block();
    drop(block.insert(Key::new(0, &observations), Value::new(1000, &observations)));
    let parent: Vec<_> = block
        .iter()
        .map(|(key, value)| (*key.data, value.id))
        .collect();
    let preimages: Vec<_> = block
        .touched_entries()
        .map(|entry| (*entry.key.data, entry.before.map(|value| value.id)))
        .collect();
    let mut transaction = without_allocations("acquire both transaction checkpoints", || {
        block.transaction()
    });
    for key in 0..128 {
        drop(transaction.remove(Key::new(key, &observations)));
    }
    transaction.insert(
        Key::new(256, &observations),
        Value::new(2560, &observations),
    );
    transaction.remove(Key::new(512, &observations));
    let clones = observations.clones();
    without_allocations("abort both private transaction trees", || drop(transaction));
    assert_eq!(observations.clones(), clones);
    assert_eq!(
        block
            .iter()
            .map(|(key, value)| (*key.data, value.id))
            .collect::<Vec<_>>(),
        parent
    );
    assert_eq!(
        block
            .touched_entries()
            .map(|entry| (*entry.key.data, entry.before.map(|value| value.id)))
            .collect::<Vec<_>>(),
        preimages
    );
    assert!(block.is_dirty());
    assert_eq!(*old.get(&0_u64).unwrap().data, 0);
    block.commit();
    assert_eq!(*storage.view().get(&0_u64).unwrap().data, 1000);
    assert_eq!(storage.view().len(), 128);
    drop(old);
    drop(storage);
    observations.assert_released();
}

#[test]
fn storage_transaction_apply_keeps_original_current_and_undo_payloads_without_allocating() {
    use mv::storage::{Storage, StorageReadOnly};

    let observations = Arc::new(Observations::default());
    let storage: Storage<Key, Value> = (0..96)
        .map(|key| {
            (
                Key::new(key, &observations),
                Value::new(key * 10, &observations),
            )
        })
        .collect();
    let original: Vec<_> = storage
        .view()
        .iter()
        .map(|(key, value)| (*key.data, value.id))
        .collect();
    let mut block = storage.block();
    let mut transaction = block.transaction();
    for key in 0..96 {
        drop(transaction.insert(
            Key::new(key, &observations),
            Value::new(9000 + key, &observations),
        ));
    }
    assert_eq!(
        transaction
            .touched_entries()
            .map(|entry| (*entry.key.data, entry.before.unwrap().id))
            .collect::<Vec<_>>(),
        original
    );
    let current: Vec<_> = transaction
        .iter()
        .map(|(key, value)| (*key.data, value.id))
        .collect();
    let block_before: Vec<_> = (0..96)
        .map(|key| {
            let key = Key::new(key, &observations);
            (*key.data, transaction.get_before_block(&key).unwrap().id)
        })
        .collect();
    let clones = observations.clones();
    without_allocations("apply both original transaction successors", || {
        transaction.apply()
    });
    assert_eq!(observations.clones(), clones);
    assert_eq!(
        block
            .iter()
            .map(|(key, value)| (*key.data, value.id))
            .collect::<Vec<_>>(),
        current
    );
    assert_eq!(
        block
            .touched_entries()
            .map(|entry| (*entry.key.data, entry.before.unwrap().id))
            .collect::<Vec<_>>(),
        block_before
    );
    let mut sibling = block.transaction();
    drop(sibling.remove(Key::new(4, &observations)));
    without_allocations(
        "abort later sibling without changing applied parent",
        || drop(sibling),
    );
    assert_eq!(
        block
            .iter()
            .map(|(key, value)| (*key.data, value.id))
            .collect::<Vec<_>>(),
        current
    );
    block.commit();
    assert_eq!(storage.view().len(), 96);
    drop(storage);
    observations.assert_released();
}

#[test]
fn caught_transaction_preimage_clone_panic_cannot_apply_partial_touches() {
    use mv::storage::{Storage, StorageReadOnly};
    use std::sync::atomic::AtomicBool;

    #[derive(Debug)]
    struct FailClone {
        value: u64,
        fail: Arc<AtomicBool>,
    }
    impl Clone for FailClone {
        fn clone(&self) -> Self {
            assert!(!self.fail.load(SeqCst), "injected preimage clone panic");
            Self {
                value: self.value,
                fail: Arc::clone(&self.fail),
            }
        }
    }
    let fail = Arc::new(AtomicBool::new(false));
    let value = |value| FailClone {
        value,
        fail: Arc::clone(&fail),
    };
    let storage: Storage<u64, FailClone> = [(0, value(10)), (1, value(20))].into_iter().collect();
    let mut block = storage.block();
    block.insert(0, value(11));
    let mut transaction = block.transaction();
    fail.store(true, SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| transaction.insert(1, value(21)))).is_err());
    fail.store(false, SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| transaction.get(&1))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| transaction.apply())).is_err());
    assert_eq!(block.get(&0).unwrap().value, 11);
    assert_eq!(block.get(&1).unwrap().value, 20);
    assert_eq!(block.touched_entries().len(), 1);
    assert_eq!(block.get_before_block(&0).unwrap().value, 10);
    assert!(block.is_dirty());
    block.commit();
    assert_eq!(storage.view().get(&1).unwrap().value, 20);
}

#[test]
fn caught_remove_query_destructor_panic_cannot_apply_partial_transaction() {
    use mv::storage::{Storage, StorageReadOnly};

    #[derive(Debug)]
    struct QueryKey {
        order: u64,
        panic_on_drop: bool,
    }
    impl Clone for QueryKey {
        fn clone(&self) -> Self {
            Self {
                order: self.order,
                panic_on_drop: false,
            }
        }
    }
    impl PartialEq for QueryKey {
        fn eq(&self, other: &Self) -> bool {
            self.order == other.order
        }
    }
    impl Eq for QueryKey {}
    impl PartialOrd for QueryKey {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for QueryKey {
        fn cmp(&self, other: &Self) -> Ordering {
            self.order.cmp(&other.order)
        }
    }
    impl Drop for QueryKey {
        fn drop(&mut self) {
            assert!(!self.panic_on_drop, "injected owned query destructor panic");
        }
    }
    let key = QueryKey {
        order: 1,
        panic_on_drop: false,
    };
    let storage: Storage<QueryKey, u64> = [(key.clone(), 10)].into_iter().collect();
    let mut block = storage.block();
    let mut transaction = block.transaction();
    assert!(
        catch_unwind(AssertUnwindSafe(|| transaction.remove(QueryKey {
            order: 1,
            panic_on_drop: true
        })))
        .is_err()
    );
    assert!(catch_unwind(AssertUnwindSafe(|| transaction.get(&key))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| transaction.apply())).is_err());
    assert_eq!(block.get(&key), Some(&10));
    assert_eq!(block.touched_entries().len(), 0);
    assert!(!block.is_dirty());
    block.commit();
    assert_eq!(storage.view().get(&key), Some(&10));
}

#[derive(Clone, Copy)]
enum DirectBlockEdit {
    Insert,
    Remove,
    Borrow,
}

fn assert_failed_block_preimage_cannot_publish(edit: DirectBlockEdit) {
    use mv::storage::{Storage, StorageReadOnly};
    use std::sync::atomic::AtomicBool;

    #[derive(Debug)]
    struct FailClone {
        value: u64,
        fail: Arc<AtomicBool>,
    }
    impl Clone for FailClone {
        fn clone(&self) -> Self {
            assert!(
                !self.fail.load(SeqCst),
                "injected block preimage clone panic"
            );
            Self {
                value: self.value,
                fail: Arc::clone(&self.fail),
            }
        }
    }
    for detach in [false, true] {
        let fail = Arc::new(AtomicBool::new(false));
        let value = |value| FailClone {
            value,
            fail: Arc::clone(&fail),
        };
        let storage: Storage<u64, FailClone> =
            [(0, value(10)), (1, value(20))].into_iter().collect();
        let before = storage.view();
        let mut block = storage.block();
        // Make the leaf private and the block dirty first. The injected panic
        // must occur in the aggregate undo clone, outside either tree cursor.
        block.insert(0, value(11));
        fail.store(true, SeqCst);
        assert!(
            catch_unwind(AssertUnwindSafe(|| match edit {
                DirectBlockEdit::Insert => {
                    block.insert(1, value(21));
                }
                DirectBlockEdit::Remove => {
                    block.remove(1);
                }
                DirectBlockEdit::Borrow => {
                    let _ = block.get_mut(&1);
                }
            }))
            .is_err()
        );
        fail.store(false, SeqCst);
        assert!(catch_unwind(AssertUnwindSafe(|| block.get(&1))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| block.iter())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| block.range(..))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| block.first_key_value())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| block.last_key_value())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| block.len())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| block.revert_map())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| block.touched_entries())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| drop(block.transaction()))).is_err());
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = block.get_mut(&0);
            }))
            .is_err()
        );
        assert!(catch_unwind(AssertUnwindSafe(|| block.insert(2, value(30)))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| block.remove(0))).is_err());
        let mut admission_called = false;
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if detach {
                    let _ = block.try_detach(|_| {
                        admission_called = true;
                        Ok::<_, ()>(())
                    });
                } else {
                    block.commit();
                }
            }))
            .is_err()
        );
        assert!(
            !admission_called,
            "failed owners cannot reach capture admission"
        );
        let after = storage.view();
        assert_eq!(after.get(&0).unwrap().value, 10);
        assert_eq!(after.get(&1).unwrap().value, 20);
        assert_eq!(before.get(&0).unwrap().value, 10);
        assert_eq!(before.get(&1).unwrap().value, 20);
    }
}

#[test]
fn caught_block_insert_preimage_panic_cannot_publish_an_unrevertible_mutation() {
    assert_failed_block_preimage_cannot_publish(DirectBlockEdit::Insert);
}

#[test]
fn caught_block_remove_preimage_panic_cannot_publish_an_unrevertible_mutation() {
    assert_failed_block_preimage_cannot_publish(DirectBlockEdit::Remove);
}

#[test]
fn caught_block_mutable_preimage_panic_cannot_reuse_or_publish_the_owner() {
    assert_failed_block_preimage_cannot_publish(DirectBlockEdit::Borrow);
}

#[test]
fn caught_block_query_destructor_panic_cannot_commit_or_detach() {
    use mv::storage::{Storage, StorageReadOnly};

    #[derive(Debug)]
    struct QueryKey {
        order: u64,
        panic_on_drop: bool,
    }
    impl Clone for QueryKey {
        fn clone(&self) -> Self {
            Self {
                order: self.order,
                panic_on_drop: false,
            }
        }
    }
    impl PartialEq for QueryKey {
        fn eq(&self, other: &Self) -> bool {
            self.order == other.order
        }
    }
    impl Eq for QueryKey {}
    impl PartialOrd for QueryKey {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for QueryKey {
        fn cmp(&self, other: &Self) -> Ordering {
            self.order.cmp(&other.order)
        }
    }
    impl Drop for QueryKey {
        fn drop(&mut self) {
            assert!(!self.panic_on_drop, "injected block query destructor panic");
        }
    }
    for remove in [false, true] {
        for detach in [false, true] {
            let key = QueryKey {
                order: 1,
                panic_on_drop: false,
            };
            let storage: Storage<QueryKey, u64> = [(key.clone(), 10)].into_iter().collect();
            let mut block = storage.block();
            // An existing first preimage leaves the second query key to be
            // destroyed locally, after the actual current-tree edit.
            block.insert(key.clone(), 11);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let query = QueryKey {
                        order: 1,
                        panic_on_drop: true,
                    };
                    if remove {
                        block.remove(query)
                    } else {
                        block.insert(query, 12)
                    }
                }))
                .is_err()
            );
            assert!(catch_unwind(AssertUnwindSafe(|| block.get(&key))).is_err());
            let mut admission_called = false;
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    if detach {
                        let _ = block.try_detach(|_| {
                            admission_called = true;
                            Ok::<_, ()>(())
                        });
                    } else {
                        block.commit();
                    }
                }))
                .is_err()
            );
            assert!(!admission_called);
            assert_eq!(storage.view().get(&key), Some(&10));
        }
    }
}

#[test]
fn caught_child_undo_cursor_panic_cannot_publish_the_healthy_current_tree() {
    use mv::storage::{Storage, StorageReadOnly};
    use std::sync::atomic::AtomicBool;

    #[derive(Debug)]
    struct FailUndoClone {
        value: u64,
        fail: Arc<AtomicBool>,
    }
    impl Clone for FailUndoClone {
        fn clone(&self) -> Self {
            assert!(
                self.value != 10 || !self.fail.load(SeqCst),
                "injected existing undo payload clone panic"
            );
            Self {
                value: self.value,
                fail: Arc::clone(&self.fail),
            }
        }
    }
    for detach in [false, true] {
        let fail = Arc::new(AtomicBool::new(false));
        let value = |value| FailUndoClone {
            value,
            fail: Arc::clone(&fail),
        };
        let storage: Storage<u64, FailUndoClone> =
            [(0, value(10)), (1, value(20))].into_iter().collect();
        let mut block = storage.block();
        block.insert(0, value(11));
        let mut transaction = block.transaction();
        fail.store(true, SeqCst);
        // The incoming preimage (20) clones successfully; copying the saved
        // undo leaf's original payload (10) fails inside only the undo cursor.
        assert!(catch_unwind(AssertUnwindSafe(|| transaction.insert(1, value(21)))).is_err());
        fail.store(false, SeqCst);
        drop(transaction);
        assert!(catch_unwind(AssertUnwindSafe(|| block.get(&0))).is_err());
        let mut admission_called = false;
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if detach {
                    let _ = block.try_detach(|_| {
                        admission_called = true;
                        Ok::<_, ()>(())
                    });
                } else {
                    block.commit();
                }
            }))
            .is_err()
        );
        assert!(!admission_called);
        let after = storage.view();
        assert_eq!(after.get(&0).unwrap().value, 10);
        assert_eq!(after.get(&1).unwrap().value, 20);
    }
}
