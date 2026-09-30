//! Cold-profile, ICU growth and stable-sort allocation custody in one fresh process.

use iroha_allocation::AllocationBudget;
use iroha_model_base::name::{MAX_NAME_BYTES, Name};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

#[derive(Clone, Copy)]
struct Observation {
    active: bool,
    requests: [usize; 32],
    count: usize,
    bytes: usize,
    live_pointer: usize,
    live_bytes: usize,
    peak: usize,
    invalid: bool,
}
impl Observation {
    const fn new() -> Self {
        Self {
            active: false,
            requests: [0; 32],
            count: 0,
            bytes: 0,
            live_pointer: 0,
            live_bytes: 0,
            peak: 0,
            invalid: false,
        }
    }
    fn request(&mut self, pointer: *mut u8, bytes: usize, previous: Option<*mut u8>) {
        if !self.active {
            return;
        }
        if let Some(previous) = previous {
            self.invalid |= self.live_pointer != previous as usize;
        } else {
            self.invalid |= self.live_pointer != 0;
        }
        if let Some(slot) = self.requests.get_mut(self.count) {
            *slot = bytes;
        } else {
            self.invalid = true;
        }
        self.count += 1;
        self.bytes += bytes;
        self.peak = self.peak.max(self.live_bytes + bytes);
        self.live_pointer = pointer as usize;
        self.live_bytes = bytes;
    }
}
thread_local! {
    static OBSERVED: Cell<Observation> = const { Cell::new(Observation::new()) };
}
fn observe(operation: impl FnOnce(&mut Observation)) {
    let _ = OBSERVED.try_with(|cell| {
        let mut value = cell.get();
        operation(&mut value);
        cell.set(value);
    });
}
struct Allocator;
#[allow(unsafe_code)]
// SAFETY: original allocation requests are delegated to System without modification.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: unchanged system allocation request.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            observe(|state| state.request(pointer, layout.size(), None));
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: unchanged system allocation request.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            observe(|state| state.request(pointer, layout.size(), None));
        }
        pointer
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, bytes: usize) -> *mut u8 {
        // SAFETY: original live allocation and requested size are forwarded.
        let result = unsafe { System.realloc(pointer, layout, bytes) };
        if !result.is_null() {
            observe(|state| state.request(result, bytes, Some(pointer)));
        }
        result
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        observe(|state| {
            if state.active {
                state.invalid |=
                    state.live_pointer != pointer as usize || state.live_bytes != layout.size();
                state.live_pointer = 0;
                state.live_bytes = 0;
            }
        });
        // SAFETY: the original allocation is returned to its allocator.
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
struct Stop;
impl Drop for Stop {
    fn drop(&mut self) {
        observe(|state| state.active = false);
    }
}
fn measured<T>(operation: impl FnOnce() -> T) -> (T, Observation) {
    OBSERVED.with(|cell| {
        assert!(!cell.get().active);
        cell.set(Observation {
            active: true,
            ..Observation::new()
        });
    });
    let stop = Stop;
    let result = operation();
    let observed = OBSERVED.with(Cell::get);
    drop(stop);
    (result, observed)
}

// One test is intentional: this binary has no earlier Name call or parallel Name
// test that could initialize the process-global profile before the first measurement.
#[test]
fn cold_profile_and_all_nfc_backings_stay_within_the_reserved_request_bound() {
    let (cold, profile) = measured(|| Name::validate_canonical("first-cold-profile"));
    cold.unwrap();
    assert_eq!(
        profile.count, 0,
        "baked profile fingerprint and OnceLock initialization allocate nothing"
    );
    assert!(!profile.invalid);

    // CharacterAndClass is one u32. Exercise the pinned standard library's
    // maximum possible stable sort geometry separately, with backing made first.
    #[derive(Clone, Copy)]
    struct SortValue(u32);
    let mut sort_values: Vec<_> = (0..(MAX_NAME_BYTES * 4) as u32)
        .rev()
        .map(SortValue)
        .collect();
    let (_, sort) = measured(|| sort_values.sort_by_key(|value| value.0));
    assert_eq!(
        sort.count, 0,
        "the complete <=1020-element sort stays on its 4 KiB stack buffer"
    );
    assert!(!sort.invalid);
    assert!(sort_values.windows(2).all(|pair| pair[0].0 <= pair[1].0));

    // Mix short ascending/descending runs and repeated keys. Pack original
    // positions into the same four-byte value so the general stable path must
    // preserve equal-key order without changing CharacterAndClass geometry.
    let mut interleaved: Vec<_> = (0..(MAX_NAME_BYTES * 4) as u32)
        .map(|position| SortValue((((position * 73) % 127) << 10) | position))
        .collect();
    assert!(
        interleaved
            .windows(2)
            .any(|pair| pair[0].0 >> 10 < pair[1].0 >> 10)
    );
    assert!(
        interleaved
            .windows(2)
            .any(|pair| pair[0].0 >> 10 > pair[1].0 >> 10)
    );
    let (_, general_sort) = measured(|| interleaved.sort_by_key(|value| value.0 >> 10));
    assert_eq!(
        general_sort.count, 0,
        "general stable sorting must use the pinned stack scratch"
    );
    assert!(!general_sort.invalid);
    assert!(
        interleaved.windows(2).all(|pair| {
            let (left_key, right_key) = (pair[0].0 >> 10, pair[1].0 >> 10);
            left_key < right_key
                || (left_key == right_key && (pair[0].0 & 1023) < (pair[1].0 & 1023))
        }),
        "equal keys preserve original positions"
    );

    let mut corpus = vec![
        "ascii".to_owned(),
        "é".to_owned(),
        "e\u{301}".to_owned(),
        "a\u{202e}b".to_owned(),
    ];
    for count in [18, 31, 32, 33, 63, 64, 65, 100, 126] {
        corpus.push(format!("q{}", "\u{301}".repeat(count)));
        corpus.push(format!("q{}\u{300}", "\u{315}".repeat(count)));
    }
    let mut saw_growth = false;
    let mut saw_rejection = false;
    for raw in corpus {
        assert!(raw.len() <= MAX_NAME_BYTES);
        let demand = Name::canonical_validation_scratch_bytes(&raw);
        let budget = AllocationBudget::new(demand);
        let lease = budget.try_reserve_bytes(demand).unwrap();
        let (outcome, observed) = measured(|| Name::validate_canonical(&raw));
        saw_rejection |= outcome.is_err();
        saw_growth |= observed.count > 1;
        assert!(
            !observed.invalid,
            "unexpected parallel or retained allocation for {raw:?}"
        );
        assert_eq!(
            observed.live_pointer, 0,
            "physical ICU backing must be freed before its caller returns"
        );
        assert_eq!(observed.live_bytes, 0);
        assert!(
            observed.bytes <= demand,
            "request sum exceeds the original reservation for {raw:?}"
        );
        assert!(
            observed.peak <= demand,
            "old/new growth peak exceeds reservation for {raw:?}"
        );
        let requests = &observed.requests[..observed.count];
        assert!(
            requests
                .iter()
                .all(|bytes| *bytes >= 32 * 4 && bytes.is_power_of_two())
        );
        assert!(
            requests.windows(2).all(|pair| pair[0] < pair[1]),
            "one SmallVec grows monotonically; extra sort storage is not permitted"
        );
        assert_eq!(
            budget.reserved_bytes(),
            demand,
            "original lease remains held after physical scratch reclamation"
        );
        drop(lease);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    assert!(saw_growth && saw_rejection);
    let (positive, observed) = measured(|| {
        let mut bytes = Vec::<u8>::with_capacity(std::hint::black_box(32));
        bytes.resize(32, 0);
        bytes.reserve_exact(std::hint::black_box(64));
        drop(bytes);
        true
    });
    assert!(positive && observed.count >= 2 && observed.live_pointer == 0 && !observed.invalid);
}
