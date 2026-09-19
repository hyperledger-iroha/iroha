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

// A scalar-only control activates this counter on its own thread after all
// fixture/writer allocation. Parallel tests and observer bookkeeping are excluded.
thread_local! {
    static ALLOCATION_COUNT: Cell<Option<usize>> = const { Cell::new(None) };
    static ALLOCATION_SIZES: Cell<[usize; 16]> = const { Cell::new([0; 16]) };
}

struct ObservedAllocator;

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
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        allocated(layout.size());
        // SAFETY: preserve the caller's requested layout and zeroing contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        allocated(size);
        // SAFETY: forward the same live allocation, old layout and new size.
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: forward the same allocation and layout without observing frees.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

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
